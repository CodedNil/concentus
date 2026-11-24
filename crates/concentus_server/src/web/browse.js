const observer = new IntersectionObserver((entries) => {
    for (const entry of entries) {
        if (entry.isIntersecting) loadMore(entry.target);
    }
}, { rootMargin: "0px 0px 320px" });

async function loadMore(form) {
    if (form.getAttribute("aria-busy") === "true") return;
    observer.unobserve(form);
    const grid = form.previousElementSibling;
    const cursor = form.elements.namedItem("after");
    const status = form.querySelector('[role="status"]');
    form.setAttribute("aria-busy", "true");
    status.textContent = "Loading more…";
    try {
        const response = await fetch("/catalog/more?" + new URLSearchParams(new FormData(form)));
        if (!response.ok) throw Error("Could not load more results");
        const html = document.createElement("template");
        html.innerHTML = await response.text();
        const batch = html.content.querySelector("[data-catalog-batch]");
        const entries = batch?.querySelector(".entries");
        if (!entries || entries.dataset.kind !== grid.dataset.kind || batch.dataset.next === cursor.value) {
            throw Error("Invalid continuation");
        }
        grid.append(...entries.children);
        if (batch.dataset.next) {
            cursor.value = batch.dataset.next;
            delete form.dataset.retry;
            status.textContent = "";
            observer.observe(form);
        } else {
            form.remove();
        }
    } catch {
        form.dataset.retry = "";
        status.textContent = "Couldn’t load more results.";
        form.querySelector("button").textContent = "Retry";
    } finally {
        form.removeAttribute("aria-busy");
    }
}

for (const form of document.querySelectorAll(".catalog-continuation")) {
    form.dataset.scroll = "";
    form.addEventListener("submit", (event) => {
        event.preventDefault();
        loadMore(form);
    });
    observer.observe(form);
}
