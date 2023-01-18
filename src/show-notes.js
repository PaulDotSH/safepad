const { invoke } = window.__TAURI__.tauri;

const notes = await invoke("get_last_modified_notes", {limit: 25});
let container = document.getElementById("container")

async function init(notes) {
    container.innerHTML = ""
    notes.forEach(note => {
        console.log(note)
        let g = document.createElement('div');
        g.setAttribute("class", "row");
        let el = createNoteElement(note)
        g.appendChild(el)
        container.appendChild(g)
    });
}
init(notes)

function createNoteElement(note) {
    let note_element = document.createElement('div');
    note_element.classList.add('note-container')
    note_element.onclick = async function () {
        await invoke("set_current_note_id", {id: note.id});
        window.location.href = `edit-note.html`
    }

    let title = document.createElement('h3');
    title.innerText = note.title
    title.classList.add('note-title')
    note_element.appendChild(title)
    
    let content = document.createElement('p');
    content.innerText = note.content
    content.classList.add('note-body')
    note_element.appendChild(content)

    let tags = document.createElement('div')
    tags.classList.add("note-tag-container")
    note.tags.forEach(tag => {
        let tag_element = document.createElement('div')
        tag_element.classList.add('note-tag')
        tag_element.innerText = tag
        tags.appendChild(tag_element)
    });
    note_element.appendChild(tags)


    let modified = document.createElement('h5');
    modified.classList.add("note-tag-modified")
    let date = new Date(note.modified)
    modified.textContent = "Last modified: " + date.toLocaleString();
    note_element.appendChild(modified)


    return note_element
}

document.getElementById("search-button").addEventListener("click", async () => {
    let params = {}
    if (document.getElementById("search-input").value) {
        params.content = document.getElementById("search-input").value;
    }
    if (document.getElementById("tags-input").value) {
        params.tags = document.getElementById("tags-input").value.split(",");
    }
    if (document.getElementById("start-date-input").value) {
        params.start = convert_date(document.getElementById("start-date-input").value)
    }
    if (document.getElementById("end-date-input").value) {
        params.end = convert_date(document.getElementById("end-date-input").value)
    }
    const notes = await invoke("search_notes", params);
    init(notes);
})

function convert_date(date) {
    return date + "T00:00:00.000000000Z"
}

//save-db-button
document.getElementById("save-db-button").addEventListener("click", async () => {
    await invoke("save_state", {});
})

document.getElementById("new-note-button").addEventListener("click", async () => {
    let date = new Date()
    const note = await invoke("add_note", { note: { title: "Title", tags: ["tag1", "tag2"], content: "Content", created: date, modified: date}});
    await invoke("set_current_note_id", { id: note.id })
    window.location.href = `edit-note.html`
})

window.addEventListener("DOMContentLoaded", () => {
    init(notes)
});
