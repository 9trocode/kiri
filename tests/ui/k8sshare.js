// Share for Kubernetes: services and pods get the containers' Share cell; the
// API server and kube-system never do; a pod is steered to its service; a
// shared service or pod that goes ends its link with a reason.
kiri.on({ type: "status", version: "0.1.4", dev: true, app: "Kiri Dev", ours: "kiri-dev", context: "kiri-dev" });
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
const eq = (a, b, m) => { if (JSON.stringify(a) !== JSON.stringify(b)) throw new Error(m + ": got " + JSON.stringify(a) + " want " + JSON.stringify(b)); };
ui.vm = "running";
ui.containers.push({ Id: "k8s1", Names: ["/kiri-k8s"], Image: "rancher/k3s:v1.37.1-k3s1", State: "running", Ports: [], Labels: { "dev.kiri.k8s": "1" }, KiriStartedAt: Date.now() / 1000 - 600 });
sh.account = { signedIn: true };
const svc = (ns, name, type, ip, ports, selector = {}) => ({ ns, name, type, clusterIP: ip, ports, selector, ingress: [], created: "2026-10-28T00:00:00Z" });
const SVCS = [
  svc("default", "kubernetes", "ClusterIP", "10.43.0.1", [{ name: "https", port: 443, targetPort: 6443, protocol: "TCP" }]),
  svc("default", "web", "LoadBalancer", "10.43.42.10", [{ port: 18080, targetPort: 80, protocol: "TCP" }], { app: "web" }),
  svc("default", "shop", "ClusterIP", "10.43.66.21", [{ name: "http", port: 80, targetPort: 8080, protocol: "TCP" }, { name: "metrics", port: 9090, protocol: "TCP" }, { name: "db", port: 5432, protocol: "TCP" }]),
  svc("default", "postgres", "ClusterIP", "10.43.9.200", [{ port: 5432, protocol: "TCP" }]),
  svc("kube-system", "kube-dns", "ClusterIP", "10.43.0.10", [{ name: "dns", port: 53, protocol: "UDP" }]),
];
const pod = (ns, name, ip, labels, owner, ports) => ({ ns, name, ip, labels, owner, status: "Running", phase: "Running", ready: "1/1", allReady: true, restarts: 0, created: "2026-10-28T00:00:00Z",
  containers: [{ name: "c", image: "nginx", ready: true, restarts: 0, state: "running", ports }] });
const PODS = [
  pod("default", "web-5fc9-cdl9f", "10.42.0.17", { app: "web" }, "ReplicaSet", [{ containerPort: 80, protocol: "TCP" }]),
  pod("default", "debug", "10.42.0.30", { run: "debug" }, undefined, [{ containerPort: 8080, protocol: "TCP" }]),
];
const lists = (services, pods) => kiri.on({ type: "k8s", state: "running", ready: true, services, pods, context: "kiri-dev" });
lists(SVCS, PODS);
const row = (name) => [...document.querySelectorAll("#content tr.row")].find((r) => r.querySelector(".name")?.textContent.trim().startsWith(name));
const cell = (name) => row(name)?.querySelector("td.col-share");
const shares = () => sent.filter((x) => x.type === "k8sshare");

show("services");
t("services have a Share column", () => eq([...document.querySelectorAll("thead th")].map((th) => th.textContent.trim()).includes("Share"), true, "header"));
t("the API server and kube-system never get a button, and say why", () => {
  eq([cell("kubernetes").textContent.trim(), !!cell("kubernetes").querySelector("button")], ["—", false], "api");
  eq(cell("kubernetes").querySelector(".dash").title.includes("API server"), true, "api why");
  eq(cell("kube-dns").textContent.trim(), "—", "kube-system");
});
t("a service with no web port gets a grey globe", () => {
  eq(!!cell("postgres").querySelector(".shc.off"), true, "grey");
  eq(cell("postgres").querySelector(".shc.off").title, "Not a web port, so it isn't shared", "why");
});
t("one web port shares straight away, to the cluster IP and service port", () => {
  sent.length = 0; cell("web").querySelector("button").click();
  eq(shares().map((x) => [x.name, x.value.ip, x.value.port, x.value.label]), [["service default/web :18080", "10.43.42.10", 18080, "web"]], "sent");
  eq(cell("web").textContent.trim(), "Sharing…", "spinner");
});
t("live: the cell reads Live and the link sits under the name", () => {
  kiri.on({ type: "share", id: 5, source: "service default/web :18080", target: "service default/web :18080", state: "live", url: "https://web-k3x9.runkiri.cloud" });
  $("sh-modal")?.remove();
  eq(cell("web").textContent.trim(), "Live", "cell");
  eq(row("web").querySelector(".addr .sh-chip").textContent.trim(), "web-k3x9.runkiri.cloud", "link under the name");
});
t("several web ports open a menu, http ticked, the database port left out", () => {
  sent.length = 0; cell("shop").querySelector("button").click();
  const m = $("k-pick");
  eq([...m.querySelectorAll("[data-kgo]")].map((b) => [b.querySelector(".p").textContent, b.getAttribute("aria-checked")]), [["80", "true"], ["9090", "false"]], "ports");
  eq(m.querySelector(".ft").textContent.includes("5432"), true, "footer");
  eq(shares().length, 0, "nothing sent yet");
  m.querySelector('[data-kgo="service default/shop :9090"]').click();
  eq(shares().map((x) => x.name), ["service default/shop :9090"], "picked port");
  eq(!!$("k-pick"), false, "menu closed");
});

show("pods");
t("a deployment's pod offers its service first", () => {
  sent.length = 0; cell("web-5fc9-cdl9f").querySelector("button").click();
  const opts = [...$("k-pick").querySelectorAll("[data-kgo]")];
  eq(opts.map((b) => b.dataset.kgo), ["service default/web :18080", "pod default/web-5fc9-cdl9f :80"], "options");
  eq(opts[0].textContent.includes("Recommended"), true, "recommended");
  document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
  eq(!!$("k-pick"), false, "Escape closes");
});
t("a pod no service points at asks once, then shares the pod's IP", () => {
  sent.length = 0; cell("debug").querySelector("button").click();
  const d = document.querySelector(".scrim .dialog");
  eq(d.querySelector("h3").textContent, "Share pod debug?", "dialog");
  d.querySelector("[data-go]").click();
  eq(shares().map((x) => [x.name, x.value.ip, x.value.port]), [["pod default/debug :8080", "10.42.0.30", 8080]], "sent");
});

t("a shared service that's deleted ends its link with why, and only Dismiss", () => {
  sent.length = 0;
  lists(SVCS.filter((s) => s.name !== "web"), PODS);
  eq(sent.filter((x) => x.type === "unshare").map((x) => x.id), ["5"], "tunnel closed");
  const it = sh.items.get(5);
  eq([it.state, it.reason], ["ended", "Ended: service deleted"], "reason");
  kiri.on({ type: "share", id: 5, state: "ended", reason: "stopped" });
  eq(sh.items.get(5)?.reason, "Ended: service deleted", "kept past the tunnel's own end");
  show("share");
  const r = document.querySelector('.sh-row[data-shrow="5"]');
  eq([...r.querySelectorAll(".sh-acts button")].map((b) => b.textContent.trim()), ["Dismiss"], "buttons");
  eq(r.querySelector(".sh-main b").textContent, "web", "named by the service");
});
t("a shared pod that restarts offers its service", () => {
  lists(SVCS, PODS);
  sent.length = 0; show("pods");
  cell("web-5fc9-cdl9f").querySelector("button").click();
  $("k-pick").querySelector('[data-kgo^="pod "]').click();
  kiri.on({ type: "share", id: 6, source: "pod default/web-5fc9-cdl9f :80", target: "pod default/web-5fc9-cdl9f :80", state: "live", url: "https://web-5fc9-cdl9f-m4c8.runkiri.cloud" });
  $("sh-modal")?.remove();
  lists(SVCS, PODS.filter((p) => p.name !== "debug" && p.name !== "web-5fc9-cdl9f"));
  eq(sh.items.get(6).reason, "Ended: pod restarted · share the service to keep a link", "reason");
  show("share");
  const b = document.querySelector('.sh-row[data-shrow="6"] [data-kshsvc]');
  eq(b.textContent.trim(), "Share service web", "button");
  sent.length = 0; b.click();
  eq(shares().map((x) => x.name), ["service default/web :18080"], "shares the service");
});
t("a service's page has Share, and Stop sharing once live", () => {
  sh.starting.clear(); show("services"); openSvc("default/shop");
  eq(document.querySelector(".dhead .who b").textContent, "shop", "page");
  eq(!!document.querySelector('.dacts [data-ksh="service|default|shop"]'), true, "share");
  kiri.on({ type: "share", id: 8, source: "service default/shop :80", target: "service default/shop :80", state: "live", url: "https://shop-a1b2.runkiri.cloud" });
  $("sh-modal")?.remove();
  eq(document.querySelector(".dacts [data-kstop]").textContent.trim(), "Stop sharing", "stop");
  eq(document.querySelector(".dhead .sh-chip").textContent.trim(), "shop-a1b2.runkiri.cloud", "chip");
});
t("a pod that declares no ports takes its service's port", () => {
  const bare = pod("default", "web-bare", "10.42.0.40", { app: "web" }, "ReplicaSet", undefined);
  lists(SVCS, [bare]); show("pods");
  sent.length = 0; cell("web-bare").querySelector("button").click();
  eq([...$("k-pick").querySelectorAll("[data-kgo]")].map((b) => b.dataset.kgo), ["service default/web :18080", "pod default/web-bare :80"], "service's target port");
  document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
});
t("a pod with no ports and no service asks which port", () => {
  const lone = pod("demo", "crashy", "10.42.0.41", { run: "crashy" }, undefined, undefined);
  lists(SVCS, [lone]); show("pods");
  eq(cell("crashy").textContent.trim(), "Share", "a button, not a grey globe");
  sent.length = 0; cell("crashy").querySelector("button").click();
  const input = $("k-pod-port");
  eq(input.tagName, "INPUT", "asks");
  input.value = "5678"; document.querySelector(".scrim [data-go]").click();
  eq(shares().map((x) => [x.name, x.value.port]), [["pod demo/crashy :5678", 5678]], "sent");
});
report(R);
