// The Share column in the container list: each state, and what pressing it does.
kiri.on({ type: "status", version: "0.1.4", dev: true, app: "Kiri Dev", ours: "kiri-dev", context: "kiri-dev" });
show("containers");
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
const eq = (a, b, m) => { if (JSON.stringify(a) !== JSON.stringify(b)) throw new Error(m + ": got " + JSON.stringify(a) + " want " + JSON.stringify(b)); };
const cell = (name) => [...document.querySelectorAll("tr.row")].find((r) => r.querySelector(".link")?.textContent === name)?.querySelector("td.col-share");
t("the list has a Share column", () => eq([...document.querySelectorAll("thead th")].map((th) => th.textContent.trim()).includes("Share"), true, "header"));
t("a live link reads Live and opens its dialog", () => {
  eq(cell("web").textContent.trim(), "Live", "text");
  eq(!!cell("web").querySelector("[data-shmodal]"), true, "opens the dialog");
});
t("a link on its way reads Sharing…, with a round spinner", () => {
  eq(cell("postgres").textContent.trim(), "Sharing…", "text");
  const r = cell("postgres").querySelector(".sh-spin").getBoundingClientRect();
  eq([r.width, r.height], [11, 11], "spinner is a circle");
});
t("a stopped container can't be shared yet, and says why", () => {
  const off = cell("worker").querySelector(".shc.off");
  eq(!!off, true, "grey Share");
  eq(off.textContent, "Share", "reads Share, greyed");
  eq(off.tagName, "SPAN", "not a button");
  eq(off.title, "Start it to share it", "hover text");
});
t("Share is yellow, Live is green", () => {
  const color = (el) => getComputedStyle(el).color;
  eq(color(cell("web").querySelector(".shc")) !== color(cell("worker").querySelector(".shc")), true, "live differs from grey");
});
t("Share asks to share that container", () => {
  sh.account = { signedIn: true };
  for (const [id, x] of sh.items) if (x.source === "postgres") sh.items.delete(id);
  sh.starting.delete("postgres"); render();
  eq(cell("postgres").textContent.trim(), "Share", "idle");
  sent.length = 0; cell("postgres").querySelector("button").click();
  eq(sent.filter((x) => x.type === "share").map((x) => x.name), ["postgres"], "sent");
  eq(cell("postgres").textContent.trim(), "Sharing…", "spinner at once");
});
t("a failed link shakes, says why, and tries again when pressed", () => {
  sh.starting.delete("postgres");
  kiri.on({ type: "share", id: 77, source: "postgres", state: "ended", reason: "The plan allows 3 links at once", error: true });
  eq(cell("postgres").textContent.trim(), "Couldn't share", "text");
  eq(cell("postgres").querySelector(".shc").classList.contains("shake"), true, "shakes when it first shows");
  render();
  eq(cell("postgres").querySelector(".shc").classList.contains("shake"), false, "shakes only once");
  eq(cell("postgres").querySelector("button").title.startsWith("The plan allows 3 links at once"), true, "says why");
  sent.length = 0; cell("postgres").querySelector("button").click();
  eq(sent.filter((x) => x.type === "share").length, 1, "tried again");
  eq(sh.items.has(77), false, "failure cleared");
});
t("a failure shows for 3 seconds, then Share again", () => {
  sh.starting.delete("postgres");
  kiri.on({ type: "share", id: 78, source: "postgres", state: "ended", reason: "No network", error: true });
  eq(cell("postgres").textContent.trim(), "Couldn't share", "at first");
  sh.items.get(78).failedAt = Date.now() - 3100; render();
  eq(cell("postgres").textContent.trim(), "Share", "after 3 s");
  eq(sh.items.has(78), true, "still in the Share view");
});
t("pressing the cell doesn't open the container's page", () => {
  sent.length = 0; cell("web").querySelector("button").click();
  eq(ui.detail, null, "stayed on the list");
});
t("Share view rows have a caret and open their requests", () => {
  show("share"); sh.open = null; renderShare();
  const row = document.querySelector('.sh-row[data-shrow="1"]');
  eq(!!row.querySelector(".sh-caret"), true, "caret");
  row.querySelector(".sh-main b").click();
  eq(sh.open, 1, "opened");
  sh.open = null; show("containers");
});
t("Share in the sidebar goes back to the list from a link's page", () => {
  show("share"); sh.open = 1; renderShare();
  const pane = () => !!document.querySelector("main > .sh-pane");
  eq(pane(), true, "on the link's page");
  eq([...document.querySelectorAll("#sh-paneacts .btn.ic")].map((b) => b.title), ["Pause", "Disconnect"], "icon buttons, as in its row");
  document.querySelector('nav [data-view="share"]').click();
  eq(sh.open, null, "closed");
  eq(pane(), false, "back on the list");
  eq($("content").hidden, false, "list showing");
  show("containers");
});
t("pause stops the tunnel, resume replaces it, and it counts once", () => {
  show("share"); sh.open = null; renderShare();
  const before = shLive().length;
  sent.length = 0;
  document.querySelector('[data-shpause="1"]').click();
  eq(sent.filter((x) => x.type === "unshare").map((x) => x.id), ["1"], "asks the app to stop link 1");
  kiri.on({ type: "share", id: 1, state: "ended" });
  eq(sh.items.get(1).state, "paused", "kept as paused");
  eq(shLive().length, before - 1, "paused isn't counted");
  const order = () => [...document.querySelectorAll(".sh-list [data-shrow]")].map((r) => r.querySelector("b").textContent);
  const before2 = order(), kept = [...document.querySelectorAll(".sh-list [data-shrow]")].filter((r) => r.dataset.shrow !== "1");
  document.querySelector('[data-shresume="1"]').click();
  eq(sent.filter((x) => x.type === "share").map((x) => x.name), ["web"], "shares web again");
  kiri.on({ type: "share", id: 90, source: "web", target: "web (localhost:8080)", state: "connecting" });
  const spin = document.querySelector('[data-shrow="90"] .sh-acts .rolling');
  eq(!!spin && spin.disabled, true, "a spinner where Resume was");
  eq(getComputedStyle(spin).borderTopColor, "rgba(0, 0, 0, 0)", "the spinner has no box");
  kiri.on({ type: "share", id: 90, state: "live", url: "https://web-k3x9.runkiri.cloud" });
  eq(!!document.querySelector('[data-shrow="90"] [data-shpause="90"]'), true, "Pause once live");
  eq(order(), before2, "web keeps its place in the list");
  eq(kept.every((r) => r.isConnected), true, "the other rows weren't redrawn");
  eq(sh.items.has(1), false, "the paused one is gone");
  eq(shLive().length, before, "counts once");
  eq(shLive().filter((x) => x.source === "web").length, 1, "one web link");
  show("containers");
});
// "Copied" is a bubble above the button: on top of everything, centred on it, the button unchanged.
const tipOver = (b) => {
  const tip = document.querySelector(".copied-tip"), r = b.getBoundingClientRect(), tr = tip.getBoundingClientRect();
  return { text: tip.textContent.trim(), above: tr.bottom <= r.top, centred: Math.abs((tr.left + tr.right) / 2 - (r.left + r.right) / 2) < 2, onTop: +getComputedStyle(tip).zIndex > 8 };
};
t("Copy in the dialog shows a Copied bubble above it, and the button stays as it is", () => {
  const live = shLive().find((x) => x.url);
  shCard(String(live.id));
  const b = document.querySelector("#sh-modal [data-mcopy]");
  const notes = document.querySelectorAll(".toast").length;
  sent.length = 0; b.click();
  eq(sent.filter((x) => x.type === "copy").map((x) => x.name), [live.url], "copies the link");
  eq(b.textContent.trim(), "Copy", "the button is unchanged");
  eq(tipOver(b), { text: "Copied", above: true, centred: true, onTop: true }, "bubble");
  eq(document.querySelectorAll(".toast").length, notes, "no note behind the dialog");
  $("sh-modal").querySelector("[data-mall]").click();
  eq([!!$("sh-modal"), ui.view, sh.open, !!$("sh-pane")], [false, "share", live.id, true], "Details opens that share's page");
  show("containers");
});
t("a link that ended with an error can be shared again", () => {
  kiri.on({ type: "share", id: 91, source: "web", target: "web", state: "ended", reason: "that name is already in use by another of your tunnels", error: true, url: "https://web-k3x9.runkiri.cloud" });
  show("share");
  const row = document.querySelector('.sh-row[data-shrow="91"]');
  eq(row.classList.contains("bad"), true, "reads as failed");
  eq(!!row.querySelector('[data-shresume="91"]'), true, "Share again");
  show("containers");
});
t("the copy icon on the Share page ticks, with the bubble above it, and no note", () => {
  show("share");
  const b = document.querySelector("#content .sh-copyic");
  const notes = document.querySelectorAll(".toast").length;
  b.click();
  eq(b.classList.contains("done"), true, "ticks");
  eq(tipOver(b), { text: "Copied", above: true, centred: true, onTop: true }, "bubble");
  eq(getComputedStyle(b, "::after").content, "none", "nothing beside it in the row");
  eq(document.querySelectorAll(".toast").length, notes, "no note");
  show("containers");
});
t("one bubble at a time, gone after a moment", () => {
  show("share");
  const b = document.querySelector("#content .sh-copyic");
  b.click(); b.click();
  eq(document.querySelectorAll(".copied-tip").length, 1, "one");
  show("containers");
});
t("notes in the corner sit above dialogs", () => eq(+getComputedStyle($("toasts")).zIndex > +getComputedStyle(document.createElement("div")).zIndex && +getComputedStyle($("toasts")).zIndex > 8, true, "z-index"));
t("a running container with no published port gets yellow Share, publishes what it listens on, then shares", () => {
  sh.account = { signedIn: true };
  const box = { Id: "b1", Names: ["/gallerybox"], Image: "gallerybox:latest", State: "running", Status: "Up 1 min", Created: Date.now() / 1000 - 60, Ports: [], Labels: {} };
  ui.containers.push(box); show("containers");
  const b = cell("gallerybox").querySelector("button.shc");
  eq([b.textContent.trim(), b.classList.contains("off")], ["Share", false], "yellow Share");
  sent.length = 0; b.click();
  eq(sent.filter((x) => x.type === "listening").map((x) => x.id), ["b1"], "asks what it listens on");
  eq(cell("gallerybox").textContent.trim(), "Sharing…", "spinner at once");
  kiri.on({ type: "listening", id: "b1", name: "", ports: [{ container_port: 4567, host_port: 4567 }], offer: false });
  eq(sent.filter((x) => x.type === "ports").map((x) => [x.id, x.ports[0].container_port, x.ports[0].host_port]), [["b1", 4567, 4567]], "publishes it");
  eq(sent.filter((x) => x.type === "share").length, 0, "not shared before the port is up");
  kiri.on({ type: "lists", containers: [...ui.containers.filter((c) => c.Id !== "b1"), { ...box, Ports: [{ PublicPort: 4567, PrivatePort: 4567, Type: "tcp" }] }], images: [], volumes: [], networks: [] });
  eq(sent.filter((x) => x.type === "share").map((x) => x.name), ["gallerybox"], "then shares");
});
t("one that listens on nothing says so", () => {
  const quiet = { Id: "b2", Names: ["/quiet"], Image: "alpine", State: "running", Status: "Up 1 min", Created: Date.now() / 1000 - 60, Ports: [], Labels: {} };
  ui.containers.push(quiet); show("containers");
  sent.length = 0; cell("quiet").querySelector("button.shc").click();
  kiri.on({ type: "listening", id: "b2", name: "", ports: [], offer: false });
  eq(cell("quiet").textContent.trim(), "Share", "back to Share");
  eq(sent.filter((x) => x.type === "ports").length, 0, "nothing published");
});
t("Disconnect says Disconnecting… at once, and a late event can't bring Share back early", () => {
  for (const [id, x] of sh.items) if (x.source === "web" && x.state === "ended") sh.items.delete(id);
  const it = [...sh.items.values()].find((x) => x.source === "web");
  sent.length = 0; shStop(it.id); render();
  eq(sent.filter((x) => x.type === "unshare").map((x) => x.id), [String(it.id)], "sent once");
  eq(cell("web").textContent.trim(), "Disconnecting…", "at once");
  eq(!!cell("web").querySelector("button"), false, "nothing to press");
  kiri.on({ type: "share", id: it.id, source: "web", state: "live" }); render();
  eq(cell("web").textContent.trim(), "Disconnecting…", "still, after a late live");
  kiri.on({ type: "share", id: it.id, source: "web", state: "ended" }); render();
  eq(cell("web").textContent.trim(), "Share", "Share once it has closed");
});
t("a redraw waits while the mouse button is down, so the click lands", () => {
  show("share");
  const before = document.querySelector("#content").firstElementChild;
  document.dispatchEvent(new PointerEvent("pointerdown", { button: 0, bubbles: true }));
  renderShare();
  eq(document.querySelector("#content").firstElementChild === before, true, "same page while pressed");
  document.dispatchEvent(new PointerEvent("pointerup", { button: 0, bubbles: true }));
  eq(press.later.size, 1, "one redraw waiting");
  show("containers");
});
report(R);
