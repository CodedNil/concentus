const form = document.querySelector("#save-record");
const save = form.querySelector("button");
const status = document.querySelector("#edit-status");
const fields = [...document.querySelectorAll("[data-field]")].map((row) => {
    const input = row.querySelector("[data-value]");
    return {
        row, input, initial: input.value,
        name: row.dataset.field,
        original: JSON.parse(row.dataset.original),
        format: row.dataset.format,
        refresh: row.querySelector("[data-refresh]"),
        language: row.querySelector("[data-language]"),
        lyrics: row.querySelector("[data-lyrics]"),
        diff: row.querySelector(".edit-diff"),
    };
});
let saving = false;

function fieldValue(field) {
    const value = field.input.value;
    if (value === field.initial) return field.original;
    if (field.format === "json") return JSON.parse(value);
    if (field.format === "number") return value === "" ? null : field.input.valueAsNumber;
    return field.format === "optional-text" && value === "" ? null : value;
}

function preview(value) {
    if (value === null || value === "") return "Empty";
    return typeof value === "string" ? value : JSON.stringify(value);
}

function previousLyrics(original, language) {
    const channels = original.lyrics_channels.filter((channel) => channel.language === language);
    const spans = channels.flatMap((channel) =>
        original.channel_spans.filter((span) => span.owner_id === channel.id).map((span) => span.text),
    );
    if (spans.length) return spans.join("\n");
    return original.track_lyrics.some((entry) => entry.language === language && entry.instrumental)
        ? "Instrumental"
        : "Empty";
}

function collect() {
    const changes = {};
    const refresh = [];
    let lyrics = null;
    let invalid = false;

    for (const field of fields) {
        const locked = field.refresh?.checked ?? false;
        let edited = false;
        field.input.disabled = locked;
        if (field.language) {
            field.language.disabled = locked;
            field.lyrics.disabled = locked;
        }
        if (locked) {
            refresh.push(field.name);
        } else {
            let valid = field.input.validity.valid;
            let value;
            try {
                value = fieldValue(field);
            } catch {
                valid = false;
            }
            edited = !valid || JSON.stringify(value) !== JSON.stringify(field.original);
            if (edited && valid) changes[field.name] = value;
            let next = valid ? preview(value) : field.input.value;
            let previous = preview(field.original);
            if (field.language && (field.language.value.trim() || field.lyrics.value)) {
                const language = field.language.value.trim();
                const text = field.lyrics.value;
                valid = valid && !edited && /^[a-z0-9]+(?:-[a-z0-9]+)*$/i.test(language) && language.length <= 35;
                edited = true;
                lyrics = { language, text };
                previous = previousLyrics(field.original, language);
                next = text.trim() ? text : "Remove lyrics";
            }
            field.diff.querySelector("del").textContent = edited ? previous : "";
            field.diff.querySelector("ins").textContent = edited ? next : "";
            field.input.setAttribute("aria-invalid", String(!valid));
            invalid ||= !valid;
        }
        if (locked) field.input.removeAttribute("aria-invalid");
        field.diff.hidden = !edited;
        field.row.toggleAttribute("data-edited", edited);
        field.row.toggleAttribute("data-locked", locked);
    }
    const count = Object.keys(changes).length + Number(Boolean(lyrics));
    save.disabled = saving || invalid || count + refresh.length === 0;
    status.textContent = invalid
        ? "Check the highlighted fields or choose one lyrics editor."
        : [count && count + (count === 1 ? " edit" : " edits"), refresh.length && refresh.length + " to refresh"].filter(Boolean).join(" · ");
    return { changes, refresh, lyrics };
}

let openHistory;
function closeHistory(restoreFocus = false) {
    if (!openHistory) return;
    const { button, panel } = openHistory;
    button.setAttribute("aria-expanded", "false");
    panel.hidden = true;
    openHistory = null;
    if (restoreFocus) button.focus();
}

function stage(field, json) {
    const value = JSON.parse(json);
    if (field.refresh) field.refresh.checked = false;
    if (field.language) {
        field.language.value = "";
        field.lyrics.value = "";
    }
    field.input.value = field.format === "json" ? JSON.stringify(value, null, 2) : value ?? "";
}

document.addEventListener("click", (event) => {
    const button = event.target.closest("button");
    if (button?.dataset.history) {
        const wasOpen = openHistory?.button === button;
        closeHistory();
        if (!wasOpen) {
            const panel = document.getElementById(button.dataset.history);
            button.setAttribute("aria-expanded", "true");
            panel.hidden = false;
            openHistory = { button, panel };
        }
        return;
    }
    if (button?.hasAttribute("data-restore-field")) {
        const field = fields.find(({ row }) => row.contains(button));
        stage(field, button.dataset.restoreField);
        collect();
        closeHistory();
        field.input.focus();
    } else if (button?.hasAttribute("data-restore-record")) {
        for (const field of fields) {
            const revision = [...field.row.querySelectorAll("[data-restore-field]")].findLast(
                (entry) => Number(entry.dataset.minute) >= Number(button.dataset.restoreRecord),
            );
            if (revision) stage(field, revision.dataset.restoreField);
        }
        collect();
        closeHistory(true);
    } else if (openHistory && !openHistory.panel.contains(event.target)) {
        closeHistory();
    }
});
document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") closeHistory(true);
});
document.addEventListener("input", (event) => {
    if (event.target.closest("[data-field]")) collect();
});

form.addEventListener("submit", async (event) => {
    event.preventDefault();
    const batch = collect();
    if (save.disabled) return;
    saving = true;
    save.disabled = true;
    status.textContent = "Saving…";
    try {
        await saveEdits(form.action, { action: "apply", value: JSON.stringify(batch) });
        window.location.reload();
    } catch (error) {
        saving = false;
        collect();
        status.textContent = "Could not save edits. " + error.message;
    }
});

collect();
