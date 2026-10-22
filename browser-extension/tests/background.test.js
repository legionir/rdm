"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");

function installChromeMock() {
  const state = {
    menus: [],
    native: [],
    notifications: [],
    collectedLinks: [],
    clicked: null
  };
  global.chrome = {
    runtime: {
      lastError: null,
      onInstalled: { addListener() {} },
      sendNativeMessage(host, request, callback) {
        state.native.push({ host, request });
        callback({ ok: true, running: true });
      }
    },
    contextMenus: {
      removeAll(callback) {
        state.menus = [];
        callback();
      },
      create(menu) {
        state.menus.push(menu);
      },
      onClicked: {
        addListener(listener) {
          state.clicked = listener;
        }
      }
    },
    tabs: {
      sendMessage(_tabId, _message, callback) {
        callback({ links: state.collectedLinks });
      }
    },
    notifications: {
      create(notification) {
        state.notifications.push(notification);
      }
    }
  };
  return state;
}

const chromeState = installChromeMock();
const background = require("../common/background.js");

test("context menus have the two requested titles", () => {
  background.installMenus();
  assert.deepEqual(
    chromeState.menus.map((menu) => menu.title),
    ["Download with RDM", "Download all links with RDM"]
  );
});

test("single-link command opens the existing New download form", () => {
  background.onMenuClicked(
    { menuItemId: "rdm-download-one", linkUrl: "https://example.com/file.zip" },
    { id: 1 }
  );
  const call = chromeState.native.at(-1);
  assert.equal(call.host, "com.legionir.rdm");
  assert.deepEqual(call.request, {
    action: "open_new",
    url: "https://example.com/file.zip"
  });
});

test("all-links command sends normalized, deduplicated HTTP links", () => {
  chromeState.collectedLinks = [
    { url: "https://example.com/a.zip", label: " A " },
    { url: "https://example.com/a.zip", label: "duplicate" },
    { url: "ftp://example.com/ignored" },
    { url: "https://example.com/b.zip" }
  ];
  background.onMenuClicked({ menuItemId: "rdm-download-all" }, { id: 7 });
  const call = chromeState.native.at(-1);
  assert.equal(call.request.action, "select_links");
  assert.deepEqual(call.request.links, [
    { url: "https://example.com/a.zip", label: "A" },
    { url: "https://example.com/b.zip" }
  ]);
});
