// Every Settings tab opens, including Import when there's nothing to import
// from (no OrbStack or Docker Desktop, or an engine that can't import yet).
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
kiri.on({ type: "importsources", sources: [] });
show("settings");
for (const id of [...document.querySelectorAll("[data-stab]")].map((b) => b.dataset.stab)) {
  t(`Settings → ${id} opens`, () => {
    document.querySelector(`[data-stab="${id}"]`).click();
    if (document.querySelector(`[data-stab="${id}"]`)?.getAttribute("aria-current") !== "true") throw new Error("tab didn't switch");
  });
}
t("Import with no sources says so", () => {
  document.querySelector('[data-stab="import"]').click();
  const text = document.querySelector(".settings .pane")?.textContent || "";
  if (!/Nothing to import/.test(text)) throw new Error("pane says: " + text.trim().slice(0, 80));
});
report(R);
