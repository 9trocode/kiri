// Every Settings tab keeps its panes in the second column, beside the tabs:
// Resources has two (the engine, then its limits), and the second one fell
// under the 180 px tab list, its sliders and words squeezed into it.
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
kiri.on({ type: "importsources", sources: [] });
kiri.on({ type: "resources", cpus: 24, memory_mib: 65536, running_cpus: 24, running_memory_mib: 65536, awake: false, max_cpus: 32, max_memory_mib: 130048, overridden: false });
show("settings");
for (const id of [...document.querySelectorAll("[data-stab]")].map((b) => b.dataset.stab)) {
  t(`Settings → ${id}: every pane beside the tabs`, () => {
    document.querySelector(`[data-stab="${id}"]`).click();
    const tabs = document.querySelector(".settings .tabs").getBoundingClientRect();
    const panes = [...document.querySelectorAll(".settings .pane")];
    if (!panes.length) throw new Error("no pane");
    for (const p of panes) {
      const r = p.getBoundingClientRect();
      if (r.left < tabs.right) throw new Error(`"${p.querySelector("h3")?.textContent}" starts at ${Math.round(r.left)}, under the tabs (they end at ${Math.round(tabs.right)})`);
    }
  });
}
t("Resources: both sliders beside the tabs, not under them", () => {
  document.querySelector('[data-stab="resources"]').click();
  const tabs = document.querySelector(".settings .tabs").getBoundingClientRect();
  for (const id of ["res-cpus", "res-mem"]) {
    const r = document.getElementById(id)?.getBoundingClientRect();
    if (!r) throw new Error(`no ${id}`);
    if (r.left < tabs.right) throw new Error(`${id} starts at ${Math.round(r.left)}, under the tabs`);
  }
});
report(R);
