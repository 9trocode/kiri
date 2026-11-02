// Star Kiri is asked once: the first thing that works, after the settings
// are in and only if ui.json doesn't say it was asked; Star opens the repo.
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
t("nothing before the settings are in", () => {
  kiri.on({ type: "pulled", name: "nginx:alpine", ms: 840 });
  if (starTimer) throw new Error("asked before prefs");
});
t("already asked: never again", () => {
  kiri.on({ type: "prefs", prefs: { starAsked: true } });
  kiri.on({ type: "k8sop", op: "start", state: "done", context: "kiri" });
  if (starTimer) throw new Error("asked twice");
});
t("the first success asks, once", () => {
  prefs.starAsked = false;
  kiri.on({ type: "pulled", name: "redis:7", ms: 1240 });
  const first = starTimer;
  kiri.on({ type: "share", id: 9, source: "web", state: "live", url: "https://web-x.runkiri.cloud" });
  if (!first || starTimer !== first) throw new Error("not asked, or asked again");
  clearTimeout(starTimer);
  starCard("Pulled redis:7 in 1.2 s.");
  const el = document.querySelector("#toasts .toast.star");
  if (!el || !/Pulled redis:7 in 1\.2 s\./.test(el.textContent) || !/helps others find it/.test(el.textContent)) throw new Error(el?.outerHTML || "no card");
  if (!sent.some((m) => m.type === "setpref" && m.name === "starAsked" && m.value === true)) throw new Error("not saved");
});
t("Star opens the repo and the card goes", () => {
  document.querySelector("#toasts .toast.star [data-star]").click();
  if (!sent.some((m) => m.type === "open" && m.name === "https://github.com/libracoder/kiri")) throw new Error("didn't open");
  if (!document.querySelector("#toasts .toast.star.leaving")) throw new Error("stayed");
});
t("a container newly running counts; ones already up don't", () => {
  starUp = null; starCheck();
  const before = starUp.size;
  ui.containers.push({ Id: "b9", Names: ["/new"], Image: "x", State: "running", Labels: {} });
  starCheck();
  if (starUp.size !== before + 1) throw new Error(`${before} → ${starUp.size}`);
});
report(R);
