const workspace = document.querySelector(".tag-workspace");
const container = workspace.querySelector(".tag-board");
const binSlot = workspace.querySelector(".tag-bin-slot");
const icon = workspace.querySelector("#tag-row-icon");
const pencil = workspace.querySelector("#tag-pencil-icon");
const status = workspace.querySelector(".tag-status");
const edit = workspace.querySelector("[data-tag-edit]");
const undo = workspace.querySelector("[data-tag-undo]");
const save = workspace.querySelector("[data-tag-save]");
const kinds = { Genre: "Genres", Mood: "Moods", Activity: "Activities", Unclassified: "Unclassified", Hidden: "Hidden bin" };
const original = JSON.parse(workspace.dataset.tags);
const filter = workspace.dataset.filter.trim().toLowerCase();
let tags;
let operations = [];
let editing = false;
let dragged;
let saving = false;
let message = "";

const children = (board, id) => board.filter((tag) => !tag.alias_of && tag.parent === id);
const changed = (tag, previous) => tag.kind !== previous.kind || tag.parent !== previous.parent || tag.alias_of !== previous.alias_of;
const compare = (a, b) => a.name.localeCompare(b.name) || a.id.localeCompare(b.id);

function move(board, tag, kind, parent) {
  let depth = 1;
  for (let key = parent; key; depth++) {
    const ancestor = board.find((item) => item.id === key);
    if (!ancestor || ancestor.alias_of || key === tag.id || ancestor.kind !== kind || depth >= 3) {
      throw Error("Tags can nest up to three levels within a single list.");
    }
    key = ancestor.parent;
  }
  function update(item, level) {
    if (level > 3) throw Error("That branch would exceed three levels.");
    item.kind = kind;
    board.filter((alias) => alias.alias_of === item.id).forEach((alias) => { alias.kind = kind; });
    children(board, item.id).forEach((child) => update(child, level + 1));
  }
  update(tag, depth);
  tag.parent = parent;
  tag.alias_of = null;
}

function apply(board, operation) {
  const tag = board.find((item) => item.id === operation.id);
  if (operation.action === "rename") {
    tag.name = operation.name;
    return board;
  }
  if (operation.action === "move") {
    move(board, tag, operation.kind, operation.parent);
    return board;
  }
  const target = board.find((item) => item.id === operation.target);
  if (target.alias_of) throw Error("Choose a canonical tag to merge into.");
  for (let node = target; node; node = board.find((item) => item.id === node.parent)) {
    if (node.id === tag.id) throw Error("A tag cannot merge into itself or one of its children.");
  }
  children(board, tag.id).forEach((child) => move(board, child, target.kind, target.id));
  for (const alias of board.filter((item) => item.id === tag.id || item.alias_of === tag.id)) {
    alias.alias_of = target.id;
    alias.parent = null;
    alias.kind = target.kind;
  }
  return board;
}

function stage(operation) {
  if (saving) return;
  try {
    tags = apply(structuredClone(tags), operation);
    operations.push(operation);
    message = "";
    render();
    container.querySelector(`[data-tag-id="${CSS.escape(operation.target || operation.id)}"] .tag-name`)?.focus();
  } catch (error) {
    status.textContent = error.message;
  }
}

function node(tag, classes, text) {
  const element = document.createElement(tag);
  if (classes) element.className = classes;
  if (text) element.textContent = text;
  return element;
}

function tagHint(id, name, details, classes) {
  const label = node("button", classes + " has-tip");
  label.type = "button";
  label.dataset.tagDetail = id;
  label.setAttribute("aria-haspopup", "dialog");
  label.append(document.createTextNode(name));
  const hint = node("span", "tip");
  hint.append(node("strong", "", name), node("small", "", details));
  label.append(hint);
  return label;
}

function button(text, action, classes) {
  const element = node("button", classes, text);
  element.type = "button";
  element.addEventListener("click", action);
  return element;
}

function dropZone(element, operation, preview = element) {
  const label = preview.textContent;
  function reset() {
    preview.textContent = label;
    element.classList.remove("tag-drop-active", "tag-drop-invalid");
  }
  element.addEventListener("tag-reset-preview", reset);
  element.addEventListener("dragover", (event) => {
    if (!dragged) return;
    event.preventDefault();
    event.stopPropagation();
    for (const active of workspace.querySelectorAll(".tag-drop-active, .tag-drop-invalid")) {
      if (active !== element) active.dispatchEvent(new Event("tag-reset-preview"));
    }
    if (element.classList.contains("tag-drop-active")) {
      event.dataTransfer.dropEffect = "move";
      return;
    }
    if (element.classList.contains("tag-drop-invalid")) {
      event.dataTransfer.dropEffect = "none";
      return;
    }
    const request = operation(dragged);
    const source = current.byId.get(dragged).name;
    try {
      apply(structuredClone(tags), request);
      const target = current.byId.get(request.target || request.parent);
      preview.textContent = request.action === "merge" ? source + " → alias of " + target.name
        : target ? source + " ↳ " + target.name : source + " → " + kinds[request.kind];
      element.classList.add("tag-drop-active");
      event.dataTransfer.dropEffect = "move";
    } catch {
      preview.textContent = "Can’t drop here";
      element.classList.add("tag-drop-invalid");
      event.dataTransfer.dropEffect = "none";
    }
  });
  element.addEventListener("dragleave", (event) => {
    if (!element.contains(event.relatedTarget)) reset();
  });
  element.addEventListener("drop", (event) => {
    if (!dragged) return;
    event.preventDefault();
    event.stopPropagation();
    const request = operation(dragged);
    const invalid = element.classList.contains("tag-drop-invalid");
    endDrag();
    if (!invalid) stage(request);
  });
  return element;
}

function endDrag() {
  dragged = null;
  workspace.classList.remove("tag-dragging");
  workspace.querySelectorAll(".tag-drop-active, .tag-drop-invalid").forEach((element) => element.dispatchEvent(new Event("tag-reset-preview")));
  workspace.querySelector(".tag-drag-source")?.classList.remove("tag-drag-source");
}

function rename(tag, name, control) {
  if (saving || name.hidden) return;
  const input = node("input", "tag-rename");
  input.type = "text";
  input.value = tag.name;
  input.setAttribute("aria-label", "Rename " + tag.name);
  input.maxLength = 160;
  name.hidden = true;
  control.disabled = true;
  name.after(input);
  function finish(commit) {
    if (!input.isConnected) return;
    const value = input.value.trim();
    input.remove();
    name.hidden = false;
    control.disabled = false;
    if (commit && value && value !== tag.name) stage({ action: "rename", id: tag.id, name: value });
  }
  input.addEventListener("blur", () => finish(true));
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter" || event.key === "Escape") {
      event.preventDefault();
      const cancel = event.key === "Escape";
      finish(!cancel);
      if (cancel) control.focus();
    }
  });
  input.focus();
  input.select();
}

function index(board) {
  const byId = new Map();
  const branches = new Map();
  const aliases = new Map();
  for (const tag of board) {
    byId.set(tag.id, tag);
    const map = tag.alias_of ? aliases : branches;
    const key = tag.alias_of || tag.parent;
    if (!map.has(key)) map.set(key, []);
    map.get(key).push(tag);
  }
  return { byId, branches, aliases };
}

const baseline = index(original);
let current;
function matches(tag, depth = 1) {
  return !filter || tag.name.toLowerCase().includes(filter)
    || (current.aliases.get(tag.id) || []).some((alias) => alias.name.toLowerCase().includes(filter))
    || (depth < 3 && (current.branches.get(tag.id) || []).some((child) => matches(child, depth + 1)));
}

function entries(kind, parent, ghostParent = false, depth = 1) {
  const list = node("ul", "tag-tree");
  const active = ghostParent ? [] : (current.branches.get(parent) || []).filter((tag) => tag.kind === kind && (editing || matches(tag)));
  const ghosts = editing ? (baseline.branches.get(parent) || []).filter((tag) => {
    const after = current.byId.get(tag.id);
    return tag.kind === kind && (ghostParent || changed(after, tag));
  }) : [];
  const rows = [...active.map((tag) => ({ tag, ghost: false })), ...ghosts.map((tag) => ({ tag, ghost: true }))];
  rows.sort((a, b) => compare(a.tag, b.tag));
  for (const { tag, ghost } of rows) {
    const row = node("li", "tag-row");
    if (ghost) {
      const remains = !current.byId.get(tag.id).alias_of;
      const placeholder = node("span", "tag-ghost");
      placeholder.append(icon.content.cloneNode(true), tag.name + (remains ? " · moved" : " · merged"));
      row.append(placeholder);
      if (depth < 3) row.append(entries(kind, tag.id, true, depth + 1));
    } else {
      const before = baseline.byId.get(tag.id);
      const aliases = current.aliases.get(tag.id) || [];
      const updated = tag.name !== before.name || changed(tag, before) || aliases.some((alias) => changed(alias, baseline.byId.get(alias.id)));
      const card = node("div", "tag-card" + (updated ? " tag-changed" : ""));
      card.dataset.tagId = tag.id;
      const line = node("div", "tag-card-line");
      const parent = current.byId.get(tag.parent);
      const details = [tag.kind];
      if (parent) details.push("Parent: " + parent.name);
      if (aliases.length) details.push("Also called " + aliases.map((alias) => alias.name).join(", "));
      const name = tagHint(tag.id, tag.name, details.join(" · "), "tag-name tag-trigger");
      line.append(icon.content.cloneNode(true));
      const names = node("span", "tag-names");
      if (tag.name !== before.name) names.append(node("del", "tag-old-name", before.name));
      names.append(name);
      line.append(names);
      if (editing) {
        const grip = node("span", "tag-grip", "⠿");
        grip.setAttribute("aria-hidden", "true");
        line.prepend(grip);
        const editName = button("", () => rename(tag, name, editName), "tag-menu");
        editName.append(pencil.content.cloneNode(true));
        editName.setAttribute("aria-label", "Rename " + tag.name);
        editName.title = "Rename " + tag.name;
        line.append(editName);
        card.draggable = true;
        card.addEventListener("dragstart", (event) => {
          if (saving || event.target.closest(".tag-menu, input")) {
            event.preventDefault();
            return;
          }
          dragged = tag.id;
          card.classList.add("tag-drag-source");
          event.dataTransfer.setData("text/plain", tag.id);
          event.dataTransfer.effectAllowed = "move";
          workspace.classList.add("tag-dragging");
        });
        card.addEventListener("dragend", endDrag);
      }
      card.append(line);
      if (aliases.length) {
        const list = node("div", "tag-aliases");
        for (const alias of aliases) {
          const item = tagHint(alias.id, alias.name, "Alias of " + tag.name, "tag-trigger");
          list.append(item);
        }
        card.append(list);
      }
      if (editing) {
        const targets = node("div", "tag-drop-targets");
        if (depth < 3) targets.append(dropZone(node("span", "tag-drop-target", "↳ Nest here"), (id) => ({ action: "move", id, kind, parent: tag.id })));
        targets.append(dropZone(node("span", "tag-drop-target", "↔ Merge as alias"), (id) => ({ action: "merge", id, target: tag.id })));
        card.append(targets);
      }
      row.append(card);
      if (depth < 3) row.append(entries(kind, tag.id, false, depth + 1));
    }
    list.append(row);
  }
  return list;
}

function render() {
  current = index(tags);
  workspace.toggleAttribute("data-editing", editing);
  edit.hidden = false;
  edit.textContent = editing ? "Cancel" : "Edit tags";
  undo.hidden = save.hidden = !editing;
  save.disabled = undo.disabled = saving || operations.length === 0;
  edit.disabled = saving;
  status.textContent = message || (editing
    ? (operations.length ? operations.length + " staged" : "")
    : filter ? "Matching “" + workspace.dataset.filter + "”" : "");
  const binOpen = binSlot.querySelector("details")?.open ?? false;
  binSlot.replaceChildren();
  if (editing) {
    const bin = node("details", "tag-bin");
    bin.dataset.tagKind = "Hidden";
    bin.open = binOpen;
    const count = tags.filter((tag) => !tag.alias_of && tag.kind === "Hidden").length;
    const summary = node("summary", "", "Hidden bin (" + count + ")");
    bin.append(dropZone(summary, (id) => ({ action: "move", id, kind: "Hidden", parent: null })));
    const items = node("div", "tag-bin-items");
    const tree = entries("Hidden", null);
    items.append(tree.childElementCount ? tree : node("p", "tag-empty", "Drop tags onto the bin to hide them."));
    bin.append(items);
    binSlot.append(bin);
  }
  container.replaceChildren();
  for (const [kind, label] of Object.entries(kinds)) {
    if (kind === "Hidden") continue;
    const column = node("section", "tag-column");
    column.dataset.tagKind = kind;
    const heading = node("header", "tag-column-header");
    heading.append(node("h2", "", label), node("span", "", String(tags.filter((tag) => !tag.alias_of && tag.kind === kind && (editing || matches(tag))).length)));
    column.append(heading);
    if (editing) {
      const preview = node("span", "tag-column-preview");
      heading.append(preview);
      dropZone(column, (id) => ({ action: "move", id, kind, parent: null }), preview);
    }
    const tree = entries(kind, null);
    if (tree.childElementCount) column.append(tree);
    else column.append(node("p", "tag-empty", filter && !editing ? "No matching tags" : "No tags yet"));
    container.append(column);
  }
}

edit.addEventListener("click", () => {
  editing = !editing;
  operations = [];
  tags = structuredClone(original);
  message = "";
  render();
});
undo.addEventListener("click", () => {
  operations.pop();
  tags = operations.reduce(apply, structuredClone(original));
  render();
});
save.addEventListener("click", async () => {
  saving = true;
  message = "Saving…";
  render();
  try {
    await saveEdits("/catalog/tags/edit", { operations: JSON.stringify(operations) });
    window.location.reload();
  } catch (error) {
    saving = false;
    message = "Could not save. " + error.message;
    render();
  }
});
tags = structuredClone(original);
render();
