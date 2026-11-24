{
  const panel = document.createElement("section");
  panel.className = "tag-details";
  panel.setAttribute("popover", "auto");
  panel.setAttribute("role", "dialog");
  panel.setAttribute("aria-label", "Tag details");
  panel.innerHTML = '<button type="button" class="tag-detail-close" aria-label="Close tag details">×</button><div class="tag-detail-meta"></div><input type="search" placeholder="Search linked records" aria-label="Search linked records" maxlength="160"><div class="tag-detail-results" aria-live="polite"></div>';
  document.body.append(panel);

  const close = panel.querySelector(".tag-detail-close");
  const meta = panel.querySelector(".tag-detail-meta");
  const search = panel.querySelector("input");
  const results = panel.querySelector(".tag-detail-results");
  let anchor, id, request, debounce;
  const motion = () => matchMedia("(prefers-reduced-motion: reduce)").matches ? "instant" : "smooth";

  function position() {
    if (!panel.matches(":popover-open") || !anchor?.isConnected) return;
    const rect = anchor.getBoundingClientRect();
    const below = rect.bottom + 8;
    panel.style.left = Math.max(8, Math.min(rect.left, innerWidth - panel.offsetWidth - 8)) + "px";
    panel.style.top = (below + panel.offsetHeight <= innerHeight - 8 ? below : Math.max(8, rect.top - panel.offsetHeight - 8)) + "px";
  }

  async function fragment(tag, options, signal) {
    const query = new URLSearchParams(options);
    const response = await fetch("/catalog/tags/" + encodeURIComponent(tag) + "/details?" + query, { signal });
    if (!response.ok) throw Error("Tag unavailable");
    const template = document.createElement("template");
    template.innerHTML = await response.text();
    return template.content;
  }

  function syncShelf(shelf) {
    const grid = shelf.querySelector(".entries");
    const [left, right] = shelf.querySelectorAll("[data-shelf-step]");
    const canScroll = grid.scrollWidth > grid.clientWidth + 2;
    left.hidden = right.hidden = !canScroll && !shelf.dataset.next;
    left.disabled = grid.scrollLeft <= 2;
    right.disabled = !shelf.dataset.next && grid.scrollLeft + grid.clientWidth >= grid.scrollWidth - 2;
  }

  async function refresh() {
    request?.abort();
    const controller = new AbortController();
    request = controller;
    try {
      const content = await fragment(id, { q: search.value }, controller.signal);
      if (controller.signal.aborted) return;
      meta.replaceChildren(content.querySelector(".tag-detail-heading"));
      results.replaceChildren(content.querySelector(".tag-shelves"));
      results.querySelectorAll(".tag-shelf").forEach((shelf) => {
        shelf.querySelector(".entries").addEventListener("scroll", () => syncShelf(shelf), { passive: true });
        syncShelf(shelf);
      });
    } catch {
      if (!controller.signal.aborted) results.textContent = "Couldn’t load linked records.";
    }
    position();
  }

  close.addEventListener("click", () => panel.hidePopover());
  panel.addEventListener("beforetoggle", (event) => {
    if (event.newState === "closed") {
      request?.abort();
      clearTimeout(debounce);
      anchor?.setAttribute("aria-expanded", "false");
    }
  });
  search.addEventListener("input", () => {
    clearTimeout(debounce);
    request?.abort();
    debounce = setTimeout(refresh, 180);
  });
  document.addEventListener("click", (event) => {
    const trigger = event.target.closest("[data-tag-detail]");
    if (!trigger) return;
    if (trigger === anchor && panel.matches(":popover-open")) {
      panel.hidePopover();
      return;
    }
    if (!panel.contains(trigger)) {
      anchor?.setAttribute("aria-expanded", "false");
      anchor = trigger;
    }
    anchor.setAttribute("aria-expanded", "true");
    id = trigger.dataset.tagDetail;
    clearTimeout(debounce);
    search.value = "";
    meta.textContent = "Loading tag…";
    results.replaceChildren();
    if (!panel.matches(":popover-open")) panel.showPopover();
    search.focus({ preventScroll: true });
    position();
    refresh();
  });

  results.addEventListener("click", async (event) => {
    const button = event.target.closest("[data-shelf-step]");
    if (!button) return;
    const shelf = button.closest(".tag-shelf");
    const grid = shelf.querySelector(".entries");
    const direction = Number(button.dataset.shelfStep);
    if (direction > 0 && shelf.dataset.next && grid.scrollLeft + grid.clientWidth >= grid.scrollWidth - 2) {
      button.disabled = true;
      try {
        const content = await fragment(id, { q: search.value, kind: shelf.dataset.shelf, after: shelf.dataset.next }, request?.signal);
        if (!shelf.isConnected) return;
        const batch = content.querySelector(".tag-shelf");
        if (batch) grid.append(...batch.querySelector(".entries").children);
        shelf.dataset.next = batch?.dataset.next || "";
        button.removeAttribute("title");
      } catch {
        button.title = "Couldn’t load more. Click to retry.";
      }
      syncShelf(shelf);
    }
    grid.scrollBy({ left: direction * grid.clientWidth, behavior: motion() });
  });
  window.addEventListener("resize", () => {
    position();
    results.querySelectorAll(".tag-shelf").forEach(syncShelf);
  });
  window.addEventListener("scroll", position, { passive: true });
}
