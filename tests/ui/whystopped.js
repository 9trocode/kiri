// A container that stops within seconds says why on its page, from its own
// log (or the engine's answer), with the fix in one step. The log text is
// what these images really print.
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
const now = Date.now() / 1000;
const ctr = (id, name, image, extra = {}) => ({ Id: id, Names: ["/" + name], Image: image, State: "exited", Status: "Exited (1) 3 seconds ago", Created: now - 5, Ports: [], Labels: {},
  KiriStartedAt: now - 4, KiriFinishedAt: now - 3, KiriExitCode: 1, ...extra });
const PGLOG = ["Error: Database is uninitialized and superuser password is not specified.", "       You must specify POSTGRES_PASSWORD to a non-empty value for the",
  "       superuser. For example, \"-e POSTGRES_PASSWORD=password\" on \"docker run\".", "", "       You may also use \"POSTGRES_HOST_AUTH_METHOD=trust\" to allow all"];
const OSLOG = ["Enabling OpenSearch Security Plugin", "OpenSearch 2.12.0 onwards, the OpenSearch Security Plugin a change that requires an initial password for 'admin' user. ",
  "Please define an environment variable 'OPENSEARCH_INITIAL_ADMIN_PASSWORD' with a strong password string. ", "If a password is not provided, the setup will quit. ",
  "No custom admin password found. Please provide a password via the environment variable OPENSEARCH_INITIAL_ADMIN_PASSWORD."];
const MYLOG = ["2026-10-29 15:34:52+00:00 [Note] [Entrypoint]: Entrypoint script for MySQL Server 26.10.0-1.el9 started.",
  "2026-10-29 15:34:52+00:00 [ERROR] [Entrypoint]: Database is uninitialized and password option is not specified", "    You need to specify one of the following as an environment variable:",
  "    - MYSQL_ROOT_PASSWORD", "    - MYSQL_ALLOW_EMPTY_PASSWORD", "    - MYSQL_RANDOM_ROOT_PASSWORD"];
const KINDLOG = ["INFO: ensuring we can execute mount/umount even with userns-remap", "INFO: remounting /sys read-only", "mount: /sys: permission denied.", "       dmesg(1) may have more information after failed mount system call."];
const lists = (cs) => kiri.on({ type: "lists", containers: cs, images: [{ Id: "sha256:" + "a".repeat(64), RepoTags: ["postgres:17"], Size: 1e8, Created: now - 9e4 }], volumes: [], networks: [] });
const holder = { Id: "h1", Names: ["/siem-postgres-1"], Image: "postgres:17", State: "running", Status: "Up 2 hours", Created: now - 9e3, Labels: {}, Ports: [{ PrivatePort: 5432, PublicPort: 5432, Type: "tcp", IP: "0.0.0.0" }] };
lists([ctr("pg1", "fast_kalam", "postgres:17"), ctr("os1", "fearless_thompson", "opensearchproject/opensearch:3.3.0"), ctr("k1", "tidy_johnson", "kindest/node:v1.33.1"), ctr("my1", "eager_mandela", "mysql:latest"),
  ctr("u1", "odd_one", "busybox"), ctr("p1", "late_port", "postgres:17", { State: "created", KiriStartedAt: null, KiriFinishedAt: null, KiriExitCode: 0 }), holder]);
const open = (id, lines) => {
  sent.length = 0;
  if (ui.view !== "containers") show("containers");
  openDetail(id);
  kiri.on({ type: "inspect", id, info: { Id: id.padEnd(64, "0"), Config: { Image: "x", Env: ["PATH=/usr/bin", "PG_MAJOR=17"] }, HostConfig: {}, State: { Running: false }, NetworkSettings: {}, Mounts: [] } });
  if (lines) kiri.on({ type: "lastlogs", id, lines });
};
const box = () => document.querySelector(".fix");
const asked = (type) => sent.filter((m) => m.type === type);

open("pg1", null);
t("a quick stop asks for the log once", () => { if (asked("lastlogs").length !== 1 || asked("lastlogs")[0].id !== "pg1") throw new Error(JSON.stringify(asked("lastlogs"))); });
kiri.on({ type: "lastlogs", id: "pg1", lines: PGLOG });
t("Postgres: it needs a password, quoting the log", () => {
  if (!/Postgres needs a password to start/.test(box()?.textContent)) throw new Error(box()?.textContent);
  if (!/You must specify POSTGRES_PASSWORD/.test(box().querySelector("q").textContent)) throw new Error(box().querySelector("q").textContent);
  if (document.querySelector("label[for=fix-val]").textContent !== "POSTGRES_PASSWORD") throw new Error("field");
  if (!document.querySelector("[data-fixsave]").disabled) throw new Error("Save on with nothing typed");
});
t("Generate fills a password and turns Save on", () => {
  document.querySelector("[data-fixgen]").click();
  const v = document.getElementById("fix-val").value;
  if (!strong(v) || v.length !== 20) throw new Error(v);
  if (document.querySelector("[data-fixsave]").disabled) throw new Error("Save still off");
});
t("Save and start keeps the environment, adds the password, then starts it", () => {
  const v = document.getElementById("fix-val").value;
  document.querySelector("[data-fixsave]").click();
  const env = asked("env")[0];
  if (!env || env.id !== "pg1" || JSON.stringify(env.value) !== JSON.stringify(["PATH=/usr/bin", "PG_MAJOR=17", `POSTGRES_PASSWORD=${v}`])) throw new Error(JSON.stringify(env));
  if (!/Starting…/.test(box().textContent) || !document.getElementById("fix-val").disabled) throw new Error("not busy");
  kiri.on({ type: "inspect", id: "pg1", info: { Id: "pg1".padEnd(64, "0"), Config: { Image: "x", Env: env.value }, HostConfig: {}, State: { Running: false }, NetworkSettings: {}, Mounts: [] } });
  if (!asked("container").some((m) => m.action === "start" && m.id === "pg1")) throw new Error("not started");
});
t("once it runs, the panel goes and says it started", () => {
  lists(ui.containers.map((c) => (c.Id === "pg1" ? { ...c, State: "running", Status: "Up 1 second", KiriExitCode: 0 } : c)));
  if (box()) throw new Error("panel stayed");
  if (!/Started with POSTGRES_PASSWORD set/.test(document.querySelector(".fixed")?.textContent)) throw new Error("no started note");
});

closeDetail(); open("os1", OSLOG);
t("OpenSearch: an admin password, strong, or security off", () => {
  if (!/OpenSearch needs an admin password to start/.test(box()?.textContent)) throw new Error(box()?.textContent);
  if (document.querySelector("label[for=fix-val]").textContent !== "OPENSEARCH_INITIAL_ADMIN_PASSWORD") throw new Error("field");
  const f = document.getElementById("fix-val");
  f.value = "weak"; f.dispatchEvent(new Event("input", { bubbles: true }));
  if (!document.querySelector("[data-fixsave]").disabled) throw new Error("weak accepted");
  f.value = "Str0ng-pass"; f.dispatchEvent(new Event("input", { bubbles: true }));
  if (document.querySelector("[data-fixsave]").disabled || !document.getElementById("fix-hint").classList.contains("ok")) throw new Error("strong refused");
  document.querySelector("[data-fixenv]").click();
  const env = asked("env").at(-1);
  if (!env.value.includes("DISABLE_SECURITY_PLUGIN=true")) throw new Error(JSON.stringify(env.value));
});

closeDetail(); open("my1", MYLOG);
t("MySQL: the first variable it lists, quoting the list", () => {
  if (!/needs a password to start/.test(box()?.textContent)) throw new Error(box()?.textContent);
  if (document.querySelector("label[for=fix-val]").textContent !== "MYSQL_ROOT_PASSWORD") throw new Error("field");
  const q = box().querySelector("q").textContent;
  if (!/password option is not specified/.test(q) || !/- MYSQL_RANDOM_ROOT_PASSWORD/.test(q)) throw new Error(q);
});
t("MySQL: or an empty password, or a random one", () => {
  const alts = [...box().querySelectorAll("[data-fixenv]")].map((b) => b.textContent);
  if (JSON.stringify(alts) !== JSON.stringify(["Or allow an empty password", "Or let it pick a random password"])) throw new Error(JSON.stringify(alts));
  box().querySelectorAll("[data-fixenv]")[1].click();
  if (!asked("env").at(-1).value.includes("MYSQL_RANDOM_ROOT_PASSWORD=yes")) throw new Error(JSON.stringify(asked("env")));
});
t("MariaDB: the names in one sentence work too", () => {
  closeDetail(); open("my1", ["2026-10-29 [ERROR] [Entrypoint]: Database is uninitialized and password option is not specified",
    "\tYou need to specify one of MARIADB_ROOT_PASSWORD, MARIADB_ROOT_PASSWORD_HASH, MARIADB_ALLOW_EMPTY_ROOT_PASSWORD and MARIADB_RANDOM_ROOT_PASSWORD"]);
  if (document.querySelector("label[for=fix-val]")?.textContent !== "MARIADB_ROOT_PASSWORD") throw new Error(box()?.textContent);
  if (box().querySelectorAll("[data-fixenv]").length !== 2) throw new Error("alts");
});

closeDetail(); open("k1", KINDLOG);
t("kindest/node: privileged mode, and Kiri's Kubernetes instead", () => {
  if (!/This image needs privileged mode/.test(box()?.textContent)) throw new Error(box()?.textContent);
  if (!/mount: \/sys: permission denied/.test(box().querySelector("q").textContent)) throw new Error("quote");
  if (document.getElementById("fix-val")) throw new Error("offers a field");
  document.querySelector("[data-fixk8s]").click();
  if (ui.view !== "pods") throw new Error("view " + ui.view);
});

open("u1", ["starting busybox", "sh: can't open 'run.sh': No such file or directory"]);
t("anything else: the last error line, as it is", () => {
  if (!/odd_one stopped after 1 second/.test(box()?.textContent)) throw new Error(box()?.textContent);
  if (!/can't open 'run.sh'/.test(box().querySelector("q").textContent)) throw new Error("quote");
  if (!document.querySelector(".fix [data-dtab=logs]")) throw new Error("no Show full log");
});

closeDetail(); open("p1", null);
kiri.on({ type: "startfailed", id: "p1", error: "driver failed programming external connectivity on endpoint late_port: Bind for 0.0.0.0:5432 failed: port is already allocated" });
t("a port taken: who has it, another port, or stop that one", () => {
  if (!new RegExp(`Port 5432 is already in use on this ${HOST}`).test(box()?.textContent)) throw new Error(box()?.textContent);
  if (!/siem-postgres-1 has it/.test(box().textContent)) throw new Error("holder");
  if (!document.querySelector('.fix [data-ports="p1"]') || !document.querySelector('.fix [data-c="stop"][data-id="h1"]')) throw new Error("actions");
});

closeDetail();
t("a running container has no panel", () => { open("h1", null); if (box()) throw new Error("panel on a running one"); });

closeDetail(); show("images");
kiri.on({ type: "notice", kind: "ok", text: "Started fast_kalam", open: "pg2", image: "postgres:17" });
lists([...ui.containers, ctr("pg2", "fast_kalam", "postgres:17")]);
kiri.on({ type: "lastlogs", id: "pg2", lines: PGLOG });
t("Images: the Started tip turns red and offers the fix", () => {
  const tip = document.querySelector(".run-tip.fail");
  if (!tip) throw new Error("no red tip: " + document.querySelector(".run-tip")?.outerHTML);
  if (!/fast_kalam stopped: needs POSTGRES_PASSWORD/.test(tip.textContent)) throw new Error(tip.textContent);
  tip.querySelector("[data-tfix]").click();
  kiri.on({ type: "inspect", id: "pg2", info: { Id: "pg2".padEnd(64, "0"), Config: { Image: "x", Env: [] }, HostConfig: {}, State: { Running: false }, NetworkSettings: {}, Mounts: [] } });
  if (ui.detail?.id !== "pg2" || !document.getElementById("fix-val")) throw new Error("didn't open the fix");
  if (document.activeElement?.id !== "fix-val") throw new Error("field not focused: " + document.activeElement?.outerHTML.slice(0, 60));
});
report(R);
