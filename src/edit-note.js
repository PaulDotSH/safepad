const { invoke } = window.__TAURI__.tauri;

const id = await invoke("get_current_note_id", {});
console.log(id)

let title_el = document.getElementById("title-input")
let content_el = document.getElementById("content-input")
let tags_el = document.getElementById("tags-input")

async function load_note(id) {
    const note = await invoke("get_note_by_id", {id: id});

    title_el.value = note.title
    content_el.value = note.content
    tags_el.value = note.tags.join()

    preview()
}

function preview() {
    console.log(content_el.value)
    document.getElementById('preview').innerHTML = marked.parse(content_el.value);
}

load_note(id)

document.getElementById("save-button").addEventListener("click", async () => {
    preview()
    console.log(tags_el.value.split(","))
    const note = await invoke("update_note",
        {id: id, title: title_el.value, content: content_el.value, tags: tags_el.value.split(",")});
});

document.getElementById("delete-button").addEventListener("click", async () => {
    await invoke("delete_note",
        {id: id});
        location.href='/show-notes.html'
});

var timeout;
content_el.addEventListener('keypress', (event) => {
    if(timeout) {
        clearTimeout(timeout);
        timeout = null;
    }

    timeout = setTimeout(() => {
        preview()
    }, 1000)
});