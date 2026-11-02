// Run without EXPOSE: the app finds what the container listens on, the page
// offers to publish it, and Publish sends the ports through the usual route.
kiri.on({ type: "status", version: "0.1.4", dev: true, app: "Kiri Dev", ours: "kiri-dev", context: "kiri-dev" });
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
const eq = (a, b, m) => { if (JSON.stringify(a) !== JSON.stringify(b)) throw new Error(m + ": got " + JSON.stringify(a) + " want " + JSON.stringify(b)); };
const text = (el) => el?.textContent.replace(/\s+/g, " ").trim() || "";
const gallerybox = { Id: "g1", Names: ["/gallerybox"], Image: "gallerybox:latest", State: "running", Status: "Up 2 seconds", Created: Date.now() / 1000, Ports: [], Labels: {} };
kiri.on({ type: "lists", containers: [...ui.containers, gallerybox], images: [], volumes: [], networks: [] });
show("containers");
const offer = () => document.querySelector(".toast[data-listen]");
t("a quiet answer says nothing", () => {
  kiri.on({ type: "listening", id: "g1", name: "gallerybox", ports: [], offer: true });
  eq(!!offer(), false, "no toast");
});
t("a listening container gets an offer naming its port", () => {
  kiri.on({ type: "listening", id: "g1", name: "gallerybox", ports: [{ container_port: 4567, host_port: 4567 }], offer: true });
  eq(text(offer().querySelector(".txt")), "gallerybox is listening on 4567Publish on localhost:4567", "toast");
});
t("Publish sends the port on the same Mac port and the offer goes", () => {
  sent.length = 0;
  offer().querySelector("[data-listenpub]").click();
  eq(sent.filter((x) => x.type === "ports").map((x) => [x.id, x.ports]), [["g1", [{ container_port: 4567, host_port: 4567, proto: "tcp", host_ip: "" }]]], "sent");
  eq([!!offer(), ui.listening.has("g1")], [false, false], "gone");
});
t("a taken Mac port is left to the engine", () => {
  kiri.on({ type: "listening", id: "g1", name: "gallerybox", ports: [{ container_port: 3000, host_port: 0 }, { container_port: 8080, host_port: 8080 }], offer: true });
  eq(text(offer().querySelector("[data-listenpub]")), "Publish on a free port, localhost:8080", "label");
  sent.length = 0; offer().querySelector("[data-listenpub]").click();
  eq(sent.filter((x) => x.type === "ports")[0].ports.map((p) => p.host_port), [0, 8080], "host ports");
});
t("the container's page asks, and its Ports row offers Publish", () => {
  sent.length = 0;
  openDetail("g1");
  eq(sent.filter((x) => x.type === "listening").map((x) => x.id), ["g1"], "asked");
  ui.detail.info = { Id: "g1aaaaaaaaaaaa", Name: "/gallerybox", Created: new Date().toISOString(), State: { Running: true, StartedAt: new Date().toISOString() },
    Config: { Image: "gallerybox:latest", Env: [], Labels: {} }, HostConfig: {}, NetworkSettings: { Networks: {} }, Mounts: [] };
  kiri.on({ type: "listening", id: "g1", name: "", ports: [{ container_port: 4567, host_port: 4567 }], offer: false });
  eq(!!offer(), false, "no toast from the page's own ask");
  const dd = [...document.querySelectorAll(".kv dt")].find((d) => d.textContent === "Ports")?.nextElementSibling;
  eq(text(dd), "Listening on 4567 · Publish", "row");
  sent.length = 0; dd.querySelector("[data-listenpub]").click();
  eq(sent.filter((x) => x.type === "ports").map((x) => x.ports[0].container_port), [4567], "sent");
});
t("a container that publishes ports isn't asked", () => {
  closeDetail(); sent.length = 0;
  openDetail("a1");
  eq(sent.filter((x) => x.type === "listening").length, 0, "not asked");
});
report(R);
