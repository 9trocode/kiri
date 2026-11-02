// Every share on the account, coloured by computer: this window's own and
// kiri share's on this Mac are green (the CLI one tagged), another computer's
// are purple External. The plan's limit counts them all.
kiri.on({ type: "status", version: "0.1.4", dev: true, app: "Kiri Dev", ours: "kiri-dev", context: "kiri-dev" });
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
const eq = (a, b, m) => { if (JSON.stringify(a) !== JSON.stringify(b)) throw new Error(m + ": got " + JSON.stringify(a) + " want " + JSON.stringify(b)); };
const text = (el) => el?.textContent.replace(/\s+/g, " ").trim();
// One link of the window's own (web), as in the design.
for (const id of [2, 3]) sh.items.delete(id);
sh.starting.clear();
ui.containers.push({ Id: "f1", Names: ["/flower"], Image: "mher/flower:2.0", State: "running", Status: "Up 1 hour", Created: Date.now() / 1000 - 3600, Ports: [{ PublicPort: 8088, PrivatePort: 5555, Type: "tcp" }], Labels: {} });
const NOW = Date.now() / 1000;
const WEB = { name: "web-k3x9ab", label: "web", url: "https://web-k3x9.runkiri.cloud", host: "paul-mbp", kind: "app", program: "Kiri Dev", target: "web (localhost:8080)", started: NOW - 840, mine: true, where: "here" };
const FLOWER = { name: "flower-demo-x1ab23", label: "flower-demo", url: "https://flower-demo-x1.runkiri.cloud", host: "paul-mbp", kind: "cli", program: "kiri share", target: "localhost:8088", started: NOW - 2520, mine: false, where: "here" };
const API = { name: "api-7fq2ab", label: "api", url: "https://api-7fq2.runkiri.cloud", host: "paul-desktop", kind: "app", program: "Kiri Dev", target: "api (localhost:3000)", started: NOW - 7500, mine: false, where: "external" };
const list = (items, limit = 3) => kiri.on({ type: "sharetunnels", items, limit });
list([WEB, FLOWER, API]);
const card = () => { shCard("list"); return $("sh-modal"); };
const rowsOf = (el) => [...el.querySelectorAll(".sh-mrow")];

t("signed in, the window polls the account's tunnels", () => {
  eq(!!sh.poll, true, "polling");
  sent.length = 0; shPoll();
  eq(sent.filter((x) => x.type === "sharetunnels").length, 1, "asks");
});
t("the card: this Mac's shares green, the CLI one tagged, the other computer's purple under External", () => {
  const c = card(), rows = rowsOf(c);
  eq(rows.map((r) => r.className), ["sh-mrow live", "sh-mrow live", "sh-mrow ext"], "colours");
  eq(text(rows[1].querySelector("b")), "flower-demoCLI", "CLI tag");
  eq(text(rows[1].querySelector("small")), "kiri share on this Mac · localhost:8088", "where it came from");
  eq(text(rows[1].querySelector(".sh-mnum")), "42 minup", "time up");
  eq(text(c.querySelector(".sh-mgroup")), "External on other computers", "heading");
  eq(text(rows[2].querySelector("b")), "apiExternal", "External tag");
  eq(text(rows[2].querySelector("small")), "Kiri Dev on paul-desktop · :3000", "machine and program");
  eq(!!rows[2].querySelector(".tile.ext"), true, "purple tile");
  eq(text(c.querySelector("h3")), "Shared 3 live", "counts all");
  eq(text(c.querySelector(".sh-tot")), "3 of 3 tunnels in use", "plan slots");
});
t("a card row this window didn't open goes to the Share page", () => {
  card().querySelector('[data-mremote="api-7fq2ab"]').click();
  eq([!!$("sh-modal"), ui.view], [false, "share"], "on Share");
  show("containers");
});
t("the Share view: 3 of 3, a slot each by computer, and the rows", () => {
  show("share"); sh.open = null; renderShare();
  const stat = document.querySelector(".sh-stat");
  eq(text(stat.querySelector(".v")), "3 of 3", "count");
  eq(text(stat.querySelector(".s")), "tunnels in use · 2 on this Mac, 1 on paul-desktop", "split");
  eq(!!stat.querySelector(".s .x"), true, "the other computer's part is purple");
  eq([...stat.querySelectorAll(".sh-slots i")].map((i) => i.className), ["app", "app", "ext"], "slots");
  eq(document.querySelector(".sh-intro").textContent.includes("every link on your account, on any computer"), true, "intro");
  const cli = document.querySelector('.sh-row.cli[data-shname="flower-demo-x1ab23"]');
  eq(cli.classList.contains("live"), true, "CLI row is green");
  eq(text(cli.querySelector(".sh-sub")), "kiri share on this Mac · localhost:8088", "CLI sub: how long it's up is on the right, not here too");
  eq([...cli.querySelectorAll(".sh-traffic > *")].map(text), ["42 min"], "CLI uptime, once, on the right");
  eq([...cli.querySelectorAll(".sh-acts button")].map((b) => b.title), ["Disconnect"], "no Pause");
  eq(text(document.querySelector(".sh-group")), "External running on other computers on your account", "group");
  const ext = document.querySelector('.sh-row.ext[data-shname="api-7fq2ab"]');
  eq(text(ext.querySelector(".sh-sub")), "Kiri Dev on paul-desktop · port 3000", "External sub");
  eq([...ext.querySelectorAll(".sh-traffic > *")].map(text), ["2 h 5 min"], "External uptime, once, on the right");
  eq([...ext.querySelectorAll(".sh-acts button")].map(text), [""], "one stop icon, like the shares here");
  eq(getComputedStyle(ext).cursor, "default", "not clickable");
  sent.length = 0; ext.querySelector("[data-shbrowse]").click();
  eq(sent.filter((x) => x.type === "sharebrowse").map((x) => x.name), ["api-7fq2ab"], "opens by name, not URL");
  eq(sh.open, null, "no requests pane");
  show("containers");
});
t("status bar: 3 shared · 1 external, a purple globe for the other computer", () => {
  renderStatus();
  const b = document.querySelector("[data-shstatus]");
  eq(text(b), "3 shared · 1 external", "text");
  eq([...b.querySelectorAll(".sh-globes .g")].map((g) => g.className), ["g", "g", "g ext"], "globes");
  eq(b.title, "3 shared · 2 on this Mac, 1 on paul-desktop", "hover");
  eq([...document.querySelectorAll('nav [data-view="share"] .sh-globes .g')].map((g) => g.className), ["g", "g", "g ext"], "sidebar");
});
t("only another computer sharing: the item still shows, purple", () => {
  const web = sh.items.get(1); sh.items.delete(1);
  list([API]); renderStatus();
  const b = document.querySelector("[data-shstatus]");
  eq([text(b), b.classList.contains("ext-only"), b.title], ["1 external", true, "1 shared, on paul-desktop"], "ext only");
  sh.items.set(1, web); list([WEB, FLOWER, API]); renderStatus();
});
t("a container whose port kiri share already shares is green Live with the CLI tag", () => {
  show("containers");
  const cell = [...document.querySelectorAll("tr.row")].find((r) => r.querySelector(".link")?.textContent === "flower").querySelector("td.col-share");
  const b = cell.querySelector(".shc");
  eq([b.classList.contains("live"), text(b)], [true, "LiveCLI"], "Live, tagged");
  eq([...cell.querySelector(".tip").childNodes].filter((n) => n.nodeName !== "BR").map(text).filter(Boolean), ["Shared by kiri share on this Mac", "flower-demo-x1.runkiri.cloud", "· port 8088 · up 42 min", "Press to see it, or disconnect it."], "hover card");
  eq(getComputedStyle(cell.querySelector(".tip")).display, "none", "only on hover");
  sent.length = 0; b.click();
  eq(sent.filter((x) => x.type === "share").length, 0, "no second tunnel");
  eq($("sh-modal")?.dataset.view, "list", "opens the card");
  $("sh-modal").remove();
});
t("the same rule matches a CLI share of the container by name", () => {
  list([WEB, { ...FLOWER, target: "flower (localhost:9999)" }, API]);
  const cell = [...document.querySelectorAll("tr.row")].find((r) => r.querySelector(".link")?.textContent === "flower").querySelector("td.col-share");
  eq(text(cell.querySelector(".shc")), "LiveCLI", "by name");
  list([WEB, { ...FLOWER, target: "10.0.0.5:8088" }, API]);
  const again = [...document.querySelectorAll("tr.row")].find((r) => r.querySelector(".link")?.textContent === "flower").querySelector("td.col-share");
  eq(text(again.querySelector(".shc")), "Share", "another host's 8088 isn't this container");
  list([WEB, FLOWER, API]);
});
t("another program on this Mac (the other Kiri app) is green too, tagged with its name", () => {
  list([WEB, FLOWER, API, { ...API, name: "docs-ab12cd", label: "docs", host: "paul-mbp", where: "here", program: "Kiri", target: "localhost:4000" }]);
  const rows = rowsOf(card());
  eq(rows.map((r) => r.className), ["sh-mrow live", "sh-mrow live", "sh-mrow live", "sh-mrow ext"], "green");
  eq(text(rows[2].querySelector(".cli-tag")), "Kiri", "tag");
  $("sh-modal").remove();
  list([WEB, FLOWER, API]);
});
t("a CLI share on this Mac disconnects straight away", () => {
  show("share"); renderShare();
  sent.length = 0;
  document.querySelector('[data-shkill="flower-demo-x1ab23"]').click();
  eq(sent.filter((x) => x.type === "sharestop").map((x) => x.name), ["flower-demo-x1ab23"], "stops it");
  eq(!!$("sh-stopext"), false, "no question");
  eq(!!document.querySelector('[data-shname="flower-demo-x1ab23"]'), false, "gone at once");
  list([WEB, API]);
  eq(sh.stopping.size, 0, "forgotten once the list agrees");
  list([WEB, FLOWER, API]);
  show("containers");
});
t("Stop on another computer's share asks first, naming that computer", () => {
  show("share"); renderShare();
  sent.length = 0;
  document.querySelector('[data-shext="api-7fq2ab"]').click();
  const d = $("sh-stopext");
  eq(text(d.querySelector("h3")), "Stop api on paul-desktop?", "title");
  eq(text(d.querySelector(".ext-where small")), "Kiri Dev on paul-desktop · port 3000", "where");
  eq(d.querySelector("p").textContent.includes("Kiri Dev there shows the share as stopped from another computer"), true, "what happens there");
  eq(text(d.querySelector("[data-go]")), "Stop on paul-desktop", "button");
  eq(sent.filter((x) => x.type === "sharestop").length, 0, "nothing yet");
  d.querySelector("[data-close]").click();
  eq(!!$("sh-stopext"), false, "Cancel closes it");
  document.querySelector('[data-shext="api-7fq2ab"]').click();
  $("sh-stopext").querySelector("[data-go]").click();
  eq(sent.filter((x) => x.type === "sharestop").map((x) => x.name), ["api-7fq2ab"], "stops it");
  list([WEB, FLOWER, API]);
  eq(!!document.querySelector('[data-shname="api-7fq2ab"]'), false, "stays gone while the edge still lists it");
  list([WEB, FLOWER]); list([WEB, FLOWER, API]);
  show("containers");
});
t("one the CLI made on the other computer says so in the question", () => {
  list([WEB, FLOWER, { ...API, kind: "cli", program: "kiri share" }]);
  shStopExt("api-7fq2ab");
  const d = $("sh-stopext");
  eq(text(d.querySelector(".ext-where small")), "kiri share on paul-desktop · port 3000", "where");
  eq(d.querySelector("p").textContent.includes("The kiri share there stops."), true, "what happens there");
  d.querySelector("[data-close]").click();
  list([WEB, FLOWER, API]);
});
t("the plan-limit error names what holds the slots, and Show shares opens the card", () => {
  sh.reveal.add("postgres"); sent.length = 0;
  kiri.on({ type: "share", id: 60, source: "postgres", target: "postgres (localhost:5432)", state: "connecting" });
  kiri.on({ type: "share", id: 60, state: "ended", reason: "your plan allows 3 tunnels at once; close one first", error: true });
  eq(sent.filter((x) => x.type === "sharetunnels").length > 0, true, "asks what holds them");
  eq(document.querySelectorAll(".toast.cap").length, 0, "waits for the list");
  list([WEB, FLOWER, API]);
  const toastEl = document.querySelector(".toast.error.cap");
  eq(text(toastEl.querySelector("b")), "Couldn't share postgres", "title");
  eq(text(toastEl.querySelector(".txt")).replace("Show sharesDetails", "").trim(), "Couldn't share postgresYour plan allows 3 tunnels at once, and all 3 are in use: web and flower-demo (kiri share) on this Mac, and api on paul-desktop.", "names them");
  eq([...toastEl.querySelectorAll(".who")].map((w) => [w.textContent, w.classList.contains("x")]), [["web", false], ["flower-demo", false], ["api", true]], "the other computer's purple");
  const cell = [...document.querySelectorAll("tr.row")].find((r) => r.querySelector(".link")?.textContent === "postgres").querySelector("td.col-share .shc");
  eq(cell.title.startsWith("Your plan allows 3 tunnels at once, and all 3 are in use: web"), true, "the red cell says the same");
  toastEl.querySelector("[data-capshow]").click();
  eq([$("sh-modal")?.dataset.view, document.querySelectorAll(".toast.cap").length], ["list", 0], "card open, toast gone");
  $("sh-modal").remove();
});
t("a Kubernetes share refused for the limit says the same", () => {
  sh.reveal.add("service default/web :18080");
  kiri.on({ type: "share", id: 61, source: "service default/web :18080", target: "service default/web :18080", state: "ended", reason: "your plan allows 3 tunnels at once; close one first", error: true });
  list([WEB, FLOWER, API]);
  eq(text(document.querySelector(".toast.cap b")), "Couldn't share service default/web :18080", "title");
  document.querySelector(".toast.cap [data-capdetails]").click();
  eq(ui.view, "share", "Details opens Share");
  show("containers");
});
t("seen from the other computer, the colours mirror", () => {
  for (const id of [60, 61]) sh.items.delete(id);
  const web = sh.items.get(1); sh.items.delete(1);
  list([{ ...API, mine: true, where: "here" }, { ...WEB, mine: false, where: "external" }, { ...FLOWER, where: "external" }]);
  sh.items.set(1, { ...web, url: API.url, source: "api" });
  const rows = rowsOf(card());
  eq(rows.map((r) => r.className), ["sh-mrow live", "sh-mrow ext", "sh-mrow ext"], "colours");
  eq(rows.slice(1).map((r) => text(r.querySelector("small"))), ["Kiri Dev on paul-mbp · :8080", "kiri share on paul-mbp · :8088"], "which program, where");
  $("sh-modal").remove();
  sh.items.set(1, web); list([WEB, FLOWER, API]);
});
t("an unchanged list doesn't redraw", () => {
  show("containers");
  const before = document.querySelector("#content table");
  list([WEB, FLOWER, API]);
  eq(document.querySelector("#content table") === before, true, "same table");
});
t("an edge without the list: own shares only, and no more asking", () => {
  kiri.on({ type: "sharetunnels", unsupported: true });
  eq([sh.remoteOff, sh.poll, sh.remote], [true, 0, null], "stopped");
  renderStatus();
  eq(text(document.querySelector("[data-shstatus]")), "1 shared", "own only");
  sent.length = 0; shPoll();
  eq(sent.filter((x) => x.type === "sharetunnels").length, 0, "doesn't ask");
  sh.remoteOff = false; shPolling();
});
t("signing out stops polling and forgets the account's tunnels", () => {
  list([WEB, FLOWER, API]);
  kiri.on({ type: "shareaccount", signedIn: false });
  eq([sh.poll, sh.remote, shOthers().length], [0, null, 0], "forgotten");
  kiri.on({ type: "shareaccount", signedIn: true, login: "octo" });
  eq(!!sh.poll, true, "polls again");
});
report(R);
