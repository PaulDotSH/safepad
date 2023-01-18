const { invoke } = window.__TAURI__.tauri;

let does_db_exist;

async function init() {
  does_db_exist = await invoke("does_db_exist");
  console.log(does_db_exist)
  if (!does_db_exist)
    document.getElementById("password-button").innerText = "Create a new database"
  else
    document.getElementById("password-button").innerText = "Open the database"

}

async function open_create_db() {
  if (does_db_exist) {
    console.log("Opening db");
    await invoke("read_save", {password: document.getElementById("password-input").value});
  } else {
    console.log("Creating db");
    await invoke("create_state", {password: document.getElementById("password-input").value});
  }
  window.location.href = "show-notes.html"
}

window.addEventListener("DOMContentLoaded", () => {
  document.getElementById("password-button").addEventListener("click", () => open_create_db());

  init()
});
