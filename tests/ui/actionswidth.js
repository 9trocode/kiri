// Saved column widths: every row's buttons stay inside the Actions column,
// counting the fade on the buttons' left.
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
const C = (id, name, img, proj, ports) => ({ Id: id, Names: ["/" + name], Image: img, State: "running", Status: "Up 1 hour", Created: Date.now() / 1000 - 60, Ports: ports || [], Labels: proj ? { "com.docker.compose.project": proj } : {} });
kiri.on({ type: "lists", containers: [C("a", "hello-compose-backend-1", "hello-compose-backend", "hello-compose"), C("b", "hello-compose-frontend-1", "hello-compose-frontend", "hello-compose", [{ PublicPort: 8088, PrivatePort: 80 }]), C("c", "buildx_buildkit_kiri-dev-builder0", "moby/buildkit:buildx-stable-1")], images: [], volumes: [], networks: [] });
document.body.style.width = "1600px";
show("containers");
t("with saved widths, the buttons stay inside Actions, on a project's row and its containers'", () => {
  const th = [...document.querySelector("#content table").tHead.rows[0].cells].filter((c) => !c.classList.contains("ck"));
  widths().containers = { cols: new Array(th.length - 1).fill(1 / (th.length - 1)), last: 150 };
  render();
  const tbl = document.querySelector("#content table.sized");
  if (!tbl) throw new Error("not sized");
  for (const r of tbl.tBodies[0].rows) {
    const td = r.cells[r.cells.length - 1], b = [...td.querySelectorAll("button")].pop();
    if (b && b.getBoundingClientRect().right > td.getBoundingClientRect().right + 0.5) throw new Error(`${r.className}: the last button ends past the column`);
  }
  delete widths().containers; render();
});
t("every column takes a share and resizes, as in Images; Name starts widest, and there's no Image column", () => {
  const ws = () => Object.fromEntries([...document.querySelector("#content table").tHead.rows[0].cells].map((c) => [c.textContent.replace(/[↑↓]/g, "").trim(), c.offsetWidth]));
  document.body.style.width = "1300px"; render(); const a = ws();
  document.body.style.width = "1700px"; render(); const b = ws();
  // Created is hidden in a narrow window (the test page's own is), so only shown columns count.
  const shown = ["State", "Ports", "Size", "Created"].filter((k) => a[k] > 0);
  if (shown.length < 3) throw new Error(`columns missing: ${JSON.stringify(a)}`);
  for (const k of ["Name", ...shown]) if (!(b[k] > a[k])) throw new Error(`${k} went from ${a[k]} to ${b[k]}`);
  for (const k of shown) if (!(b.Name > b[k])) throw new Error(`Name ${b.Name} is not wider than ${k} ${b[k]}`);
  if ("Image" in b) throw new Error(`an Image column: ${b.Image}`);
  for (const k of ["state", "ports", "size"]) if (!document.querySelector(`#content th[data-sort="${k}"] .col-resize`)) throw new Error(`${k} has no resize edge`);
  document.body.style.width = "1600px"; render();
});
t("docker build's BuildKit worker isn't listed with the containers", () => {
  if (document.querySelector('#content tr[data-key="c"]')) throw new Error("buildx_buildkit_kiri-dev-builder0 is listed");
  if (ui.containers.some((c) => c.Id === "c")) throw new Error("kept in the list");
  if ($("n-containers").textContent !== "2") throw new Error(`count says ${$("n-containers").textContent}`);
});
report(R);
