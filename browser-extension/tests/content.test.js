"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const { pageLinks } = require("../common/content.js");

test("content script lists anchors and areas with absolute URLs", () => {
  const anchors = [
    { href: "https://example.com/a.zip", innerText: " Release A ", textContent: "ignored" },
    { href: "https://example.com/b.zip", innerText: "", textContent: " Release B " }
  ];
  let selector = "";
  const links = pageLinks({
    querySelectorAll(value) {
      selector = value;
      return anchors;
    }
  });
  assert.equal(selector, "a[href], area[href]");
  assert.deepEqual(links, [
    { url: "https://example.com/a.zip", label: "Release A" },
    { url: "https://example.com/b.zip", label: "Release B" }
  ]);
});
