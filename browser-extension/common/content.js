"use strict";

function pageLinks(documentObject) {
  return Array.from(documentObject.querySelectorAll("a[href], area[href]"), (anchor) => ({
    url: anchor.href,
    label: (anchor.innerText || anchor.textContent || "").trim().slice(0, 300)
  }));
}

if (typeof chrome !== "undefined" && chrome.runtime && chrome.runtime.onMessage) {
  chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
    if (message && message.type === "rdm.collect-links") {
      sendResponse({ links: pageLinks(document) });
    }
    // The response is synchronous; no channel needs to remain open.
    return false;
  });
}

if (typeof module !== "undefined" && module.exports) {
  module.exports = { pageLinks };
}
