// The environment: Edit in place on the container's page, then Apply sends
// the whole list to the engine, which restarts the container with it.
kiri.on({ type: "status", version: "0.1.4", dev: true, app: "Kiri Dev", ours: "kiri-dev", context: "kiri-dev" });
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
const eq = (a, b, m) => { if (JSON.stringify(a) !== JSON.stringify(b)) throw new Error(m + ": got " + JSON.stringify(a) + " want " + JSON.stringify(b)); };
const text = (el) => el?.textContent.replace(/\s+/g, " ").trim() || "";
const type = (el, v) => { el.value = v; el.dispatchEvent(new Event("input", { bubbles: true })); };
const row = (i) => document.querySelector(`[data-envi="${i}"]`);
show("containers"); openDetail("a1");
ui.detail.info = { Id: "a1aaaaaaaaaaaa", Name: "/gallerybox", Created: new Date().toISOString(), State: { Running: true, StartedAt: new Date().toISOString() },
  Config: { Image: "gallerybox:latest", Env: ["PATH=/usr/bin", "PORT=4567", "NODE_ENV=production"], Labels: {} }, HostConfig: {}, NetworkSettings: { Networks: {} }, Mounts: [] };
render();
t("the list reads as before, with Edit", () => eq([text(document.querySelector(".envhead")), document.querySelectorAll(".envs .mono").length], ["3 variablesEdit", 3], "list"));
t("Edit opens a row per variable and an empty one", () => {
  document.querySelector("[data-envedit]").click();
  eq([document.querySelectorAll(".env-r").length, row(1).querySelector(".v").value, text($("env-chg"))], [4, "4567", "No changes"], "editor");
});
t("a changed value, a new one and a removed one are counted", () => {
  type(row(1).querySelector(".v"), "9099");
  type(row(3).querySelector(".k"), "LOG_LEVEL");
  type(row(3).querySelector(".v"), "debug");
  row(2).querySelector("[data-envx]").click();
  eq([row(1).className, row(2).className, row(3).className, document.querySelectorAll(".env-r").length], ["env-r chg", "env-r del", "env-r new", 5], "rows");
  eq(text($("env-chg")), "3 changes · 1 added, 1 changed, 1 removed", "count");
});
t("typing keeps the cursor where it was", () => {
  const v = row(1).querySelector(".v"); v.focus(); v.setSelectionRange(2, 2);
  render();
  eq([document.activeElement === row(1).querySelector(".v"), document.activeElement.selectionStart], [true, 2], "focus");
});
t("pasting lines into a name fills rows", () => {
  const k = row(4).querySelector(".k"), dt = new DataTransfer();
  dt.setData("text/plain", "# comment\nexport A=1\nB=\"two words\"\nPORT=1234");
  k.dispatchEvent(new ClipboardEvent("paste", { clipboardData: dt, bubbles: true, cancelable: true }));
  eq(ui.detail.env.filter((r) => r.k).map((r) => `${r.k}=${r.v}`), ["PATH=/usr/bin", "PORT=1234", "NODE_ENV=production", "LOG_LEVEL=debug", "A=1", "B=two words"], "rows");
});
t("Apply shows the change and Restart sends the whole list", () => {
  document.querySelector("[data-envapply]").click();
  const dlg = document.querySelector(".scrim .dialog");
  eq(text(dlg.querySelector("h3")), "Restart gallerybox with the new environment?", "title");
  eq([...dlg.querySelectorAll(".env-diff > span")].map((s) => s.className), ["c", "a", "a", "a", "d"], "diff");
  sent.length = 0; dlg.querySelector("[data-go]").click();
  eq(sent.filter((x) => x.type === "env").map((x) => [x.id, x.value]), [["a1", ["PATH=/usr/bin", "PORT=1234", "LOG_LEVEL=debug", "A=1", "B=two words"]]], "sent");
  eq([ui.detail.env, !!document.querySelector(".scrim")], [null, false], "closed");
});
t("Cancel puts the list back", () => {
  document.querySelector("[data-envedit]").click(); type(row(0).querySelector(".v"), "x");
  document.querySelector("[data-envcancel]").click();
  eq([ui.detail.env, document.querySelectorAll(".envs .mono").length], [null, 3], "back");
});
t("a name with a space is refused", () => {
  document.querySelector("[data-envedit]").click(); type(row(3).querySelector(".k"), "BAD NAME"); type(row(3).querySelector(".v"), "1");
  sent.length = 0; document.querySelector("[data-envapply]").click();
  eq([!!document.querySelector(".scrim"), sent.filter((x) => x.type === "env").length], [false, 0], "refused");
  document.querySelector("[data-envcancel]").click();
});
report(R);
