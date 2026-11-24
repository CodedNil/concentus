async function saveEdits(url, fields) {
  const response = await fetch(url, {
    method: "POST",
    body: new URLSearchParams(fields),
  });
  if (!response.ok) throw new Error(await response.text());
}
