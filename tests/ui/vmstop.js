// A container that was running when the Linux VM stopped reads Stopped, not
// Failed (255): Docker's code says the app died, but nothing failed.
kiri.on({ type: "status", version: "0.1.4", dev: true, app: "Kiri Dev", ours: "kiri-dev", context: "kiri-dev" });
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
const eq = (a, b, m) => { if (JSON.stringify(a) !== JSON.stringify(b)) throw new Error(m + ": got " + JSON.stringify(a) + " want " + JSON.stringify(b)); };
const at = Math.floor(Date.now() / 1000) - 120;
const c = (id, extra) => ({ Id: id, Names: [`/${id}`], Image: "nginx", State: "exited", Status: "", KiriExitCode: 255, KiriFinishedAt: at, Ports: [], Labels: {}, ...extra });
show("containers");
kiri.on({ type: "lists", containers: [c("vm", { KiriVmStopped: true }), c("app", {})], images: [], volumes: [], networks: [] });
const pill = (id) => document.querySelector(`[data-live="${id}"]`);
t("stopped with the VM reads Stopped, uncoloured, and says why on hover", () => {
  eq([pill("vm").textContent, pill("vm").className.includes("failed"), pill("vm").title], ["Stopped", false, "Stopped 2 min ago, when the Linux VM stopped. Start it again whenever you like."], "vm");
});
t("an app that really exited 255 still reads Failed", () => {
  eq([pill("app").textContent, pill("app").className.includes("failed")], ["Failed (255)", true], "app");
});
report(R);
