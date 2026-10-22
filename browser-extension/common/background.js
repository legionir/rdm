"use strict";

const HOST_NAME = "com.legionir.rdm";
const SINGLE_MENU = "rdm-download-one";
const ALL_MENU = "rdm-download-all";

function installMenus() {
  chrome.contextMenus.removeAll(() => {
    // Reading lastError is required even when there is nothing useful to do.
    if (chrome.runtime.lastError) {
      console.warn("RDM could not reset its context menus", chrome.runtime.lastError);
    }
    chrome.contextMenus.create({
      id: SINGLE_MENU,
      title: "Download with RDM",
      contexts: ["link", "image", "video", "audio"]
    });
    chrome.contextMenus.create({
      id: ALL_MENU,
      title: "Download all links with RDM",
      contexts: ["all"]
    });
  });
}

function notify(message) {
  chrome.notifications.create({
    type: "basic",
    iconUrl: "icon.png",
    title: "RDM",
    message
  });
}

function sendToRdm(request) {
  chrome.runtime.sendNativeMessage(HOST_NAME, request, (response) => {
    const error = chrome.runtime.lastError;
    if (error) {
      notify(`Could not contact RDM. Install the native host, then reload the extension. ${error.message}`);
      return;
    }
    if (!response || !response.ok) {
      notify((response && response.error) || "RDM rejected the browser request.");
      return;
    }
    if (response.running === false) {
      notify("Request saved. Open RDM to show the download window.");
    }
  });
}

function normaliseLinks(rawLinks) {
  const seen = new Set();
  const links = [];
  for (const raw of Array.isArray(rawLinks) ? rawLinks : []) {
    const value = typeof raw === "string" ? { url: raw } : raw;
    if (!value || typeof value.url !== "string") {
      continue;
    }
    try {
      const parsed = new URL(value.url);
      if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
        continue;
      }
      if (seen.has(parsed.href)) {
        continue;
      }
      seen.add(parsed.href);
      const label = typeof value.label === "string" ? value.label.trim().slice(0, 300) : "";
      links.push({ url: parsed.href, ...(label ? { label } : {}) });
      if (links.length === 5000) {
        break;
      }
    } catch (_) {
      // Invalid and non-HTTP links are not downloadable by RDM.
    }
  }
  return links;
}

function collectAllLinks(tab) {
  if (!tab || typeof tab.id !== "number") {
    notify("RDM could not identify the active tab.");
    return;
  }
  chrome.tabs.sendMessage(tab.id, { type: "rdm.collect-links" }, (result) => {
    const error = chrome.runtime.lastError;
    if (error) {
      notify("Reload the page, then try again. Browser-internal pages do not allow link collection.");
      return;
    }
    const links = normaliseLinks(result && result.links);
    if (links.length === 0) {
      notify("No downloadable HTTP/HTTPS links were found on this page.");
      return;
    }
    sendToRdm({ action: "select_links", links });
  });
}

function onMenuClicked(info, tab) {
  if (info.menuItemId === SINGLE_MENU) {
    const url = info.linkUrl || info.srcUrl;
    if (url) {
      sendToRdm({ action: "open_new", url });
    }
  } else if (info.menuItemId === ALL_MENU) {
    collectAllLinks(tab);
  }
}

if (typeof chrome !== "undefined" && chrome.runtime && chrome.runtime.onInstalled) {
  chrome.runtime.onInstalled.addListener(installMenus);
  chrome.contextMenus.onClicked.addListener(onMenuClicked);
}

// CommonJS export is used only by the dependency-free Node tests.  Browser
// workers/event pages take the branch above and ignore this block.
if (typeof module !== "undefined" && module.exports) {
  module.exports = { installMenus, normaliseLinks, onMenuClicked, sendToRdm };
}
