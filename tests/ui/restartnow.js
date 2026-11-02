// Resources saved while the VM runs: the toast's Restart now restarts it.
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
kiri.on({ type: "notice", kind: "ok", text: "Saved. The VM uses these from its next start", act: "restartvm" });
t("the toast has Restart now", () => {
  const b = document.querySelector("#toasts [data-trestart]");
  if (!b) throw new Error("no link");
  b.click();
  if (!sent.some((m) => m.type === "restartvm")) throw new Error("sent " + sent.map((m) => m.type).join(","));
  if (document.querySelector("#toasts [data-trestart]")) throw new Error("toast stayed");
});
kiri.on({ type: "notice", kind: "ok", text: "Saved. The VM uses these from its next start", act: "" });
t("no link while the VM is asleep", () => {
  if (document.querySelector("#toasts [data-trestart]")) throw new Error("link shown");
});
report(R);
