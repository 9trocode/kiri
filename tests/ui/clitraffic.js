// A `kiri share` started in a terminal: the window reads what it has served
// from its traffic file (`sharetraffic`) and counts it in its row and in
// Traffic served, as it does its own shares.
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
kiri.on({ type: "status", version: "0.1.4", dev: true, app: "Kiri Dev", ours: "kiri-dev", context: "kiri-dev" });
kiri.on({ type: "shareaccount", signedIn: true, login: "you", session: "ks_1" });
for (const id of [...sh.items.keys()]) sh.items.delete(id);
sh.served = { requests: 2, in: 100, out: 1000 };
const DOCK = { name: "dock-4kcdss", label: "dock", url: "https://dock-4kcdss.runkiri.cloud", host: "mac", kind: "cli", program: "kiri share", target: "localhost:8790", started: Date.now() / 1000 - 47, mine: false, where: "here" };
kiri.on({ type: "sharetunnels", items: [DOCK], limit: 5 });
show("share");
const row = () => document.querySelector(`[data-shname="${DOCK.name}"]`);
const served = () => [...document.querySelectorAll(".sh-stat")].find((s) => /Traffic served/.test(s.textContent))?.textContent.replace(/\s+/g, " ");
t("before its file has anything, the row says how long it's been up", () => {
  // The time on its own, on the right, and not again under the name.
  const right = row()?.querySelector(".sh-traffic")?.textContent.trim() || "";
  if (!/^\d+ (s|min|h)/.test(right) || /live/.test(row()?.querySelector(".sh-sub")?.textContent || "")) throw new Error(row()?.textContent);
});
t("the window asks for the traffic files while a kiri share runs", () => {
  sent.length = 0; shTick();
  if (!sent.some((m) => m.type === "sharetraffic")) throw new Error(JSON.stringify(sent));
});
t("its traffic shows in its row and in Traffic served", () => {
  kiri.on({ type: "sharetraffic", items: [{ url: DOCK.url, requests: 3, in: 300, out: 2048 }] });
  const r = row()?.querySelector(".sh-traffic")?.textContent || "";
  if (!/2(\.0)? KB/.test(r) || !/3 requests/.test(r)) throw new Error(r);
  if (!/5 requests/.test(served()) || !/3(\.0)? KB/.test(served())) throw new Error(served());
});
t("new traffic moves the bars", () => {
  const before = sh.wave.at(-1);
  kiri.on({ type: "sharetraffic", items: [{ url: DOCK.url, requests: 4, in: 400, out: 4096 }] });
  if (sh.wave.at(-1) - before !== 2148) throw new Error(sh.wave.at(-1) - before);
});
report(R);
