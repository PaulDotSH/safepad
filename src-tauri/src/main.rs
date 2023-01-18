use serde_with::serde_as;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use anyhow::{anyhow, Error, Ok};
use chacha20poly1305::aead::rand_core::RngCore;
use chacha20poly1305::{XChaCha20Poly1305, KeyInit, aead::Aead};
use rand::rngs::OsRng;
use chrono::prelude::*;
use std::path::{Path, PathBuf};
use std::process::{Command, Child};
use std::sync::{Arc};
use rand::{distributions::Alphanumeric, Rng};
use tokio::sync::Mutex;
use surrealdb_rs::net::WsClient;
use surrealdb_rs::param::Root;
use surrealdb_rs::protocol::Ws;
use surrealdb_rs::StaticClient;
use surrealdb_rs::Surreal;
use once_cell::sync::Lazy;

lazy_static! {
    static ref DB_PASSWORD: String = generate_random_str(8);
}

static STATE: Lazy<Arc<Mutex<State>>> = Lazy::new(|| {
    Arc::new(Mutex::new(State::default()))
});


#[tauri::command(async)]
async fn does_db_exist() -> bool {
    does_file_exist(&SAVE_PATH.lock().await.to_path_buf()).await
}

//TODO: Change this to tokio's way
#[tauri::command(async)]
async fn does_file_exist(path: &PathBuf) -> bool {
    std::path::Path::exists(std::path::Path::new(&path))
}

fn pw_to_bytes(password: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    let result = hasher.finalize();
    result.try_into().unwrap()
}

#[tauri::command(async)]
async fn create_state(password: String) {
    *STATE.lock().await = State::new(pw_to_bytes(&password));
    STATE.lock().await.add_note( &DB,
        Note { title: "This is an example note".to_string(), tags: ["foo".to_string(), "bar".to_string()].try_into().unwrap(),
    content: "This is an example note to show you how an example note looks like".to_string(), created: chrono::Utc::now(), modified: chrono::Utc::now() }).await.expect("Cannot add example note");
}

#[tauri::command(async)]
async fn save_state() {
    STATE.lock().await.save_data_file(
        std::path::Path::new(&SAVE_PATH.lock().await.to_path_buf()),
        &DB).await.expect("Couldn't save state");
}

#[tauri::command(async)]
async fn get_last_modified_notes(limit: u64) -> Vec<IDNote> {
    let state = STATE.lock().await;
    let notes = state.get_notes(&DB, limit).await.expect("Couldn't get all notes");
    println!("{:?}", notes);
    notes
}

#[tauri::command(async)]
async fn get_all_notes() -> Vec<IDNote> {
    let state = STATE.lock().await;
    let notes = state.get_all_notes(&DB).await.expect("Couldn't get all notes");
    notes
}

#[tauri::command(async)]
async fn set_current_note_id(id: String) {
    let mut state = STATE.lock().await;
    state.note_id = id;
}

#[tauri::command(async)]
async fn get_note_by_id(id: String) -> IDNote {
    let state = STATE.lock().await;
    state.get_note(&DB, id).await.expect("Couldn't get note")
}

#[tauri::command(async)]
async fn get_current_note_id() -> String {
    let state = STATE.lock().await;
    state.note_id.clone()
}

//add_note
#[tauri::command(async)]
async fn add_note(note: Note) -> IDNote {
    let mut state = STATE.lock().await;
    state.add_note(&DB, note).await.expect("Couldn't add note")
}

#[tauri::command(async)]
async fn delete_note(id: String) {
    let mut state = STATE.lock().await;
    state.delete_note(&DB, id).await.expect("Couldn't delete note")
}


#[tauri::command(async)]
async fn update_note(id: String, title: String, content: String, tags: Vec<String>) {
    let mut state = STATE.lock().await;
    state.update_note(&DB, id, title, content, tags).await.expect("Cannot update note")
}

#[tauri::command(async)]
async fn search_notes(content: Option<String>, start: Option<DateTime<Utc>>, end: Option<DateTime<Utc>>, tags: Option<Vec<String>>) -> Vec<IDNote> {
    println!("{:?} {:?} {:?} {:?}", content, start, end, tags);
    let state = STATE.lock().await;
    let notes = state.get_notes_query(&DB, content, start, end, tags, 50).await.expect("Couldn't get notes");
    notes
}

#[tauri::command(async)]
async fn read_save(password: String) {
    let mut state = STATE.lock().await;
    (*state).password = pw_to_bytes(&password); 
    state.read_data_file(std::path::Path::new(&SAVE_PATH.lock().await.to_path_buf()), &DB).await.expect("Couldn't read data file");
}

static SAVE_PATH: Lazy<Arc<Mutex<PathBuf>>> = Lazy::new(|| {
    Arc::new(Mutex::new(PathBuf::new()))
});

static DB: Surreal<WsClient> = Surreal::new();

use sha2::{Sha256, Digest};


#[tokio::main]
async fn main() {
    // TODO: Check if surreal is already started
    start_surreal_db(&DB_PASSWORD).unwrap();

    DB.connect::<Ws>("localhost:8000").await.unwrap();

    DB.signin(Root {
        username: "safepad",
        password: &DB_PASSWORD,
    })
        .await.unwrap();

    DB.use_ns("namespace").use_db("database").await.unwrap();

    tauri::async_runtime::set(tokio::runtime::Handle::current());
    let foo = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![add_note, delete_note, search_notes, get_last_modified_notes, does_db_exist, save_state, update_note, create_state, get_note_by_id, get_all_notes, read_save, set_current_note_id, get_current_note_id]);
    let context = tauri::generate_context!();
    let app = foo.build(context).unwrap();

    let path = app.path_resolver().app_data_dir().unwrap();
    std::fs::create_dir_all(&path).expect("Cannot create db dir");
    let path=  Path::join(app.path_resolver().app_data_dir().unwrap().as_path(), "database");
    println!("The database is/will be located at {}", &path.display());
    
    *SAVE_PATH.lock().await = path;
    app.run(|_, _| {})
}

fn encrypt_bytes(data: &[u8], key: &[u8; 32], nonce: &[u8; 24]) -> Result<Vec<u8>, anyhow::Error> {
    let cipher = XChaCha20Poly1305::new(key.into());

    let mut encrypted = cipher
        .encrypt(nonce.into(), data)
        .map_err(|err| anyhow!("Encrypting bytes: {}", err))?;
    let mut v = Vec::from(nonce.as_slice());
    v.append(&mut encrypted);
    Ok(v)
}

fn decrypt_bytes(data: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, anyhow::Error> {
    let cipher = XChaCha20Poly1305::new(key.into());

    let dec = cipher
        .decrypt(data[0..24].into(), data[24..].as_ref())
        .map_err(|err| anyhow!("Decrypting bytes: {}", err))?;
    Ok(dec)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct State {
    nonce: [u8; 24],
    password: [u8; 32],
    note_id: String,
}

impl State {
    async fn update_note(&mut self, db: &Surreal<WsClient>, id: String, title: String, content: String, tags: Vec<String>) -> Result<(), Error> {
        let content = content.replace("'", "\\'");
        let tags = format!("{:?}", tags).replace("\"", "'");
        db.query(
            format!("UPDATE {} SET title = '{}', content = '{}', tags = {}, modified = '{}'", id, title, content, tags,  chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Nanos, true))
        ).await?;
        Ok(())
    }

    async fn delete_note(&mut self, db: &Surreal<WsClient>, id: String) -> Result<(), Error> {
        db.query(
            format!("DELETE {};", id)
        ).await?;
        Ok(())
    }

    async fn add_note(&mut self, db: &Surreal<WsClient>, note: Note) -> Result<IDNote, Error> {
        let result: IDNote = db
        .create("note").content(note).await?;
        Ok(result)
    }

    async fn get_note(&self, db: &Surreal<WsClient>, id: String) -> Result<IDNote, Error> {
        let resp = db.query(
            format!("SELECT * FROM note WHERE id = \"{}\";", id)
        ).await?;
        let mut resp: Vec<IDNote> = resp.get(0, ..)?;
        Ok(resp.remove(0)) 
    }

    fn default() -> Self {
        State { nonce: [0u8; 24], password: [0u8; 32], note_id: String::new() }
    }

    fn new(password: [u8; 32]) -> Self {
        State { nonce: generate_nonce(), password, note_id: String::new() }
    }

    async fn read_data_file(&mut self, path: &Path, db: &Surreal<WsClient>) -> Result<(), anyhow::Error> {
        let mut file = File::open(path).await?;
        let mut nonce = [0u8; 24];
        let mut buff: Vec<u8> = Vec::new();
        file.read_to_end(&mut buff).await?;
        for i in 0..24 {
            nonce[i] = buff[i]
        }
        let decrypted = decrypt_bytes(&buff, &self.password)?;
        drop(buff);

        let decrypted = String::from_utf8(decrypted).unwrap();
        // todo: check why it didn't work with bincode, and use something faster than json
        let notes: Vec<Note> = serde_json::from_str(&decrypted).unwrap();
        // let notes: Vec<Note> = bincode::deserialize(&decrypted)?;
        for note in notes {
            self.add_note(db, note).await?;
        }
        drop(decrypted);
        self.nonce = nonce;
        Ok(())
    }

    async fn save_data_file(&self, path: &Path, db: &Surreal<WsClient>) -> Result<(), anyhow::Error> {
        // let encoded: Vec<u8> = bincode::serialize(&self.get_all_notes(db).await?)?;
        let encoded = serde_json::to_string(&self.get_all_notes(db).await?).expect("foo");
        let encoded = encoded.as_bytes();
        let encrypted = encrypt_bytes(&encoded, &self.password, &self.nonce)?;
        drop(encoded);
        std::fs::write(path, encrypted)?;
        Ok(())
    }

    async fn get_notes(&self,  db: &Surreal<WsClient>, limit: u64) -> Result<Vec<IDNote>, Error> {
        let resp = db.query(format!("SELECT * FROM note ORDER BY modified DESC LIMIT {};", limit)).await?;
        Ok(resp.get(0, ..)?)
    }

    async fn get_notes_query(&self, db: &Surreal<WsClient>, content: Option<String>, start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>, tags: Option<Vec<String>>, limit: u64) -> Result<Vec<IDNote>, Error> {
            if content.is_none() && start.is_none() && end.is_none() && tags.is_none() {
                return self.get_notes(db, limit).await;
            }
            let mut query = "SELECT * FROM note WHERE".to_owned();

            let mut params = Vec::new();

            if content.is_some() {
                params.push(format!("content ~ \'{}\'", content.unwrap()));
            }

            if start.is_some() {
                params.push(format!("modified >= \'{}\'", start.unwrap()));
            }

            if end.is_some() {
                params.push(format!("modified <= \'{}\'", end.unwrap()));
            }

            if tags.is_some() {
                for tag in tags.unwrap() {
                    params.push(format!("tags ?~ \'{}\'", tag))
                }
            }

            query = format!("{} {} ORDER BY modified DESC LIMIT {};", query, params.join(" AND "), limit);

            println!("------------\n{:?}\n------------", query);
            let resp = db.query(
               query
            ).await?;
            Ok(resp.get(0, ..)?) 
    }

    async fn get_notes_containing(&self, db: &Surreal<WsClient>, content: String, limit: u64) -> Result<Vec<IDNote>, Error> {
        let resp = db.query(
            format!("SELECT * FROM note WHERE content ~ \"{}\" ORDER BY modified DESC LIMIT {};", content, limit)
        ).await?;
        Ok(resp.get(0, ..)?) 
    }

    async fn get_all_notes(&self, db: &Surreal<WsClient>) -> Result<Vec<IDNote>, Error> {
        let resp = db.query("SELECT * FROM note;").await?;
        Ok(resp.get(0, ..)?)
    }
}

fn generate_nonce() -> [u8; 24] {
    let mut nonce = [0u8; 24];
    OsRng.fill_bytes(&mut nonce);
    nonce
}


fn generate_random_str(length: usize) -> String {
    rand::thread_rng().sample_iter(&Alphanumeric).take(length).map(char::from).collect()
}

// We start the surreal db instance in memory to protect the data (nothing is saved on the disk)
fn start_surreal_db(password: &str) -> Result<Child, anyhow::Error> {
    Ok(Command::new(format!("surreal")).args(["start", "--user", "safepad", "--pass", password, "memory"]).spawn()?)
}

#[derive(Debug, Serialize, Deserialize)]
struct IDNote {
    id: String,
    title: String,
    tags: Vec<String>,
    content: String,
    // #[serde(serialize_with = "to_ts")]
    created: DateTime<Utc>,
    // #[serde(serialize_with = "to_ts")]
    modified: DateTime<Utc>,
}

// use chrono::serde::ts_seconds::serialize as to_ts;
#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct Note {
    title: String,
    tags: Vec<String>,
    content: String,
    // #[serde(serialize_with = "to_ts")]
    created: DateTime<Utc>,
    // #[serde(serialize_with = "to_ts")]
    modified: DateTime<Utc>,
}