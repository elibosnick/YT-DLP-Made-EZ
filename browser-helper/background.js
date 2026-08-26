chrome.action.onClicked.addListener(async (tab) => {
  if (!tab.id) return;
  const [{ result }] = await chrome.scripting.executeScript({
    target: { tabId: tab.id },
    func: () => {
      const urls = [...document.querySelectorAll("video, source")]
        .map((node) => node.currentSrc || node.src)
        .filter((url) => /^https?:/i.test(url));
      const resources = performance.getEntriesByType("resource")
        .map((entry) => entry.name)
        .filter((url) => /\.(mp4|m3u8|mpd)(\?|$)/i.test(url));
      return [...new Set([...urls, ...resources])][0] || location.href;
    },
  });
  await fetch("http://127.0.0.1:32123/media", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ url: result }),
  });
});
