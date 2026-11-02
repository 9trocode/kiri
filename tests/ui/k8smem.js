// Kubernetes views say when the VM is too small, or too full, for it.
kiri.on({ type: "status", version: "0.1.4", dev: true, app: "Kiri Dev", ours: "kiri-dev", context: "kiri-dev" });
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
const eq = (a, b, m) => { if (JSON.stringify(a) !== JSON.stringify(b)) throw new Error(m + ": got " + JSON.stringify(a) + " want " + JSON.stringify(b)); };
ui.vm = "running";
ui.containers.push({ Id: "k8s1", Names: ["/kiri-k8s"], Image: "rancher/k3s:v1.37.1-k3s1", State: "running", Ports: [], Labels: { "dev.kiri.k8s": "1" }, KiriStartedAt: Date.now() / 1000 - 600 });
const banner = () => document.querySelector("#content .kmem")?.textContent.replace(/\s+/g, " ").trim() || "";
t("a 1 GB VM says Kubernetes needs 2 GB", () => {
  ui.mem = { used: 921, target: 1024, ceiling: 1024 };
  show("pods");
  eq(banner().includes("Kubernetes needs about 2.0 GB, and this VM can use 1.0 GB, of which 921 MB is in use."), true, banner());
  eq(document.querySelector("#content .kmem [data-kres]").textContent, "Give the VM more memory", "button");
});
t("a big VM that's nearly full says so", () => {
  ui.mem = { used: 7600, target: 8192, ceiling: 8192 };
  show("services");
  eq(banner().includes("nearly full: 7.4 GB of 8.0 GB"), true, banner());
});
t("enough memory: nothing to say", () => { ui.mem = { used: 3000, target: 8192, ceiling: 8192 }; show("pods"); eq(banner(), "", "banner"); });
t("the button opens Settings → Resources", () => {
  ui.mem = { used: 921, target: 1024, ceiling: 1024 }; show("pods");
  document.querySelector("#content [data-kres]").click();
  eq([ui.view, prefs.tab], ["settings", "resources"], "where");
});
report(R);
