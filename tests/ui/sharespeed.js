// The Share view's cards with no traffic, with some, and near the plan's
// speed limit (design: kiri/share-speed-card, variant C).
show("share");
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
const eq = (a, b, m) => { if (JSON.stringify(a) !== JSON.stringify(b)) throw new Error(m + ": got " + JSON.stringify(a) + " want " + JSON.stringify(b)); };
const card = (k) => [...document.querySelectorAll(".sh-stat")].find((s) => s.querySelector(".k")?.textContent.includes(k));
const text = (el) => el.textContent.replace(/\s+/g, " ").trim();

t("no traffic: one quiet line that says so, no row of stubs", () => {
  const w = card("Traffic served").querySelector(".sh-wave");
  eq(w.classList.contains("quiet"), true, "quiet");
  eq(w.querySelectorAll("i").length, 0, "no bars");
  eq(text(w), "No visitors in the last 2 minutes", "label");
  eq(text(card("Traffic served").querySelector(".s")), "No requests yet", "sub");
});
t("no traffic: the speed is 0 B/s, idle, against the plan's limit", () => {
  const c = card("This month");
  eq(c.classList.contains("idle"), true, "idle");
  eq(text(c.querySelector(".speedrow .now")), "0 B/s", "speed");
  eq(text(c.querySelector(".lbl-limit")), "2.0 MB/s limit", "limit");
  eq(text(c.querySelector(".foot .calm")), "idle · nobody is visiting", "foot");
  eq(text(c.querySelector(".row .s")), "of 5.0 GB", "month");
});
t("a request: bars for that moment only, and the count", () => {
  kiri.on({ type: "share", id: 1, request: { method: "GET", path: "/", status: 200, ms: 12, in: 4000, out: 400000 } });
  const bars = [...card("Traffic served").querySelectorAll(".sh-wave i")].map((i) => parseInt(i.style.height));
  eq(bars.filter((h) => h > 0).length, 1, "one bar");
  eq(bars.at(-1), 34, "full height, the newest");
  eq(text(card("Traffic served").querySelector(".s")), "1 request · 4.0 KB received", "sub");
});
t("after a tick: the speed, in and out, and the peak", () => {
  shTick();
  const c = card("This month");
  eq(c.classList.contains("idle"), false, "live");
  eq(text(c.querySelector(".speedrow .now")), "101.0 KB/s", "speed");
  eq(text(c.querySelector(".foot .calm")), "↓ 1.0 · ↑ 100.0 KB/s", "in and out");
  eq(!!c.querySelector(".sbar b"), true, "peak tick");
  eq(c.classList.contains("warn"), false, "not near the limit");
});
t("near the limit: amber and a hint", () => {
  kiri.on({ type: "share", id: 1, request: { method: "GET", path: "/big", status: 200, ms: 900, in: 1000, out: 7.2e6 } });
  shTick();
  const c = card("This month");
  eq(c.classList.contains("warn"), true, "warn");
  eq(getComputedStyle(c.querySelector(".foot .hint")).display, "flex", "hint shown");
});
t("quiet again: the speed drops back to idle, the peak stays", () => {
  shTick();
  const c = card("This month");
  eq(c.classList.contains("idle"), true, "idle");
  eq(c.classList.contains("warn"), false, "not amber");
  eq(/peak 1\.8 MB\/s/.test(text(c.querySelector(".foot"))), true, "peak: " + text(c.querySelector(".foot")));
});
report(R);
