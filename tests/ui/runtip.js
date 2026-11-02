// Images → Run: once the container has stayed up a few seconds, "Started …" appears beside that image's Run button, with
// View container; off the Images view it is the toast.
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
const name = "nginx:alpine";
kiri.on({ type: "lists", containers: ui.containers, images: [{ Id: "sha256:" + "a".repeat(64), RepoTags: [name], Size: 52e6, Created: Date.now() / 1000 - 3600 }], volumes: [], networks: [] });
show("images");
t("the list has a Run button for it", () => { if (!document.querySelector(`[data-run="${CSS.escape(name)}"]`)) throw new Error("no Run for " + name); });
kiri.on({ type: "notice", kind: "ok", text: "Started brave_hopper", open: "n1", image: name });
runSay("n1"); // still up a few seconds later
t("the tip is beside the button, not a toast", () => {
  const tip = document.querySelector(".run-tip");
  if (!tip) throw new Error("no tip");
  if (!/Started brave_hopper/.test(tip.textContent)) throw new Error(tip.textContent);
  if (!tip.querySelector("svg")) throw new Error("no icon");
  const b = document.querySelector(`[data-run="${CSS.escape(name)}"]`).getBoundingClientRect(), r = tip.getBoundingClientRect();
  if (r.right > b.left || Math.abs((r.top + r.bottom) / 2 - (b.top + b.bottom) / 2) > 3) throw new Error(`tip at ${r.left},${r.top} button at ${b.left},${b.top}`);
  if (document.querySelector("#toasts [data-tview]")) throw new Error("a toast too");
});
t("View container opens it", () => {
  document.querySelector(".run-tip [data-tview]").click();
  if (ui.view !== "containers") throw new Error("view " + ui.view);
  if (document.querySelector(".run-tip")) throw new Error("tip stayed");
});
kiri.on({ type: "notice", kind: "ok", text: "Started calm_euler", open: "n2", image: name });
runSay("n2");
t("off Images it is the toast", () => {
  if (document.querySelector(".run-tip")) throw new Error("tip on " + ui.view);
  if (!document.querySelector("#toasts [data-tview]")) throw new Error("no toast");
});
report(R);
