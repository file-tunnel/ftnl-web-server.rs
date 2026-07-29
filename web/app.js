(() => {
  "use strict";

  const config = globalThis.__FTNL_CONFIG__;
  const tunnelId = location.pathname.match(/^\/t\/([0-9a-f-]{36})$/i)?.[1];
  const secret = new URLSearchParams(location.hash.slice(1)).get("c");
  const sessionKey = tunnelId ? `ftnl.phone.${tunnelId}` : "";
  const filesInput = document.querySelector("#files");
  const picker = document.querySelector("#picker");
  const queue = document.querySelector("#queue");
  const error = document.querySelector("#error");
  const connection = document.querySelector("#connection");
  const done = document.querySelector("#done");
  let capability = tunnelId ? sessionStorage.getItem(sessionKey) : null;
  let activeUploads = 0;

  // Remove the credential before any user interaction, screenshot, or copied URL.
  if (location.hash) history.replaceState(null, "", location.pathname + location.search);

  const api = (path) => `${config.apiOrigin}${path}`;
  const showError = (message) => {
    error.textContent = message;
    error.hidden = false;
    picker.hidden = true;
    connection.textContent = "Connection unavailable";
  };

  const request = async (path, options = {}) => {
    const headers = new Headers(options.headers);
    if (capability) headers.set("authorization", `Bearer ${capability}`);
    const response = await fetch(api(path), { ...options, headers });
    if (!response.ok) {
      let detail = `Request failed (${response.status})`;
      try {
        const problem = await response.json();
        if (problem.detail) detail = problem.detail;
      } catch {
        // Keep a status-only error when the edge returned non-JSON.
      }
      throw new Error(detail);
    }
    return response;
  };

  const claim = async () => {
    if (!tunnelId) throw new Error("This link does not contain a valid tunnel.");
    if (capability) return;
    if (!secret) throw new Error("This pairing link is incomplete or has already been used.");
    const response = await request(`/v1/tunnels/${tunnelId}/claim`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ pairing_secret: secret }),
    });
    const payload = await response.json();
    capability = payload.phone_capability;
    sessionStorage.setItem(sessionKey, capability);
  };

  const uploadRow = (file) => {
    const item = document.createElement("li");
    item.className = "upload";
    const row = document.createElement("div");
    row.className = "upload-row";
    const name = document.createElement("span");
    name.className = "upload-name";
    name.textContent = file.name;
    const state = document.createElement("span");
    state.className = "upload-state";
    state.textContent = "Waiting";
    const track = document.createElement("div");
    track.className = "track";
    const bar = document.createElement("div");
    bar.className = "bar";
    bar.setAttribute("role", "progressbar");
    bar.setAttribute("aria-valuemin", "0");
    bar.setAttribute("aria-valuemax", "100");
    bar.setAttribute("aria-valuenow", "0");
    row.append(name, state);
    track.append(bar);
    item.append(row, track);
    queue.append(item);
    return {
      progress(percent, label) {
        const safe = Math.max(0, Math.min(100, Math.round(percent)));
        bar.style.width = `${safe}%`;
        bar.setAttribute("aria-valuenow", String(safe));
        state.textContent = label;
      },
      complete() {
        item.classList.add("complete");
        this.progress(100, "Sent");
      },
      fail() {
        item.classList.add("failed");
        state.textContent = "Failed";
      },
    };
  };

  const putBytes = (url, file, row) =>
    new Promise((resolve, reject) => {
      const xhr = new XMLHttpRequest();
      xhr.open("PUT", url);
      xhr.setRequestHeader("authorization", `Bearer ${capability}`);
      xhr.setRequestHeader("content-type", "application/octet-stream");
      xhr.upload.addEventListener("progress", (event) => {
        if (event.lengthComputable) {
          row.progress((event.loaded / event.total) * 100, `${Math.round((event.loaded / event.total) * 100)}%`);
        }
      });
      xhr.addEventListener("load", () => {
        if (xhr.status >= 200 && xhr.status < 300) resolve();
        else reject(new Error(`Upload failed (${xhr.status})`));
      });
      xhr.addEventListener("error", () => reject(new Error("The network connection was interrupted.")));
      xhr.addEventListener("abort", () => reject(new Error("Upload cancelled.")));
      xhr.send(file);
    });

  const upload = async (file) => {
    const row = uploadRow(file);
    activeUploads += 1;
    picker.setAttribute("aria-busy", "true");
    try {
      row.progress(0, "Preparing");
      const response = await request(`/v1/tunnels/${tunnelId}/files`, {
        method: "POST",
        headers: {
          "content-type": "application/json",
          "idempotency-key": crypto.randomUUID(),
        },
        body: JSON.stringify({
          name: file.name,
          media_type: file.type || "application/octet-stream",
          size_bytes: file.size,
          last_modified_ms: file.lastModified,
        }),
      });
      const descriptor = await response.json();
      await putBytes(
        api(`/v1/tunnels/${tunnelId}/files/${descriptor.file_id}/content`),
        file,
        row,
      );
      row.complete();
    } catch (cause) {
      row.fail();
      showError(cause instanceof Error ? cause.message : "Upload failed.");
      picker.hidden = false;
    } finally {
      activeUploads -= 1;
      if (activeUploads === 0) {
        picker.removeAttribute("aria-busy");
        done.hidden = queue.children.length === 0;
      }
    }
  };

  filesInput.addEventListener("change", () => {
    error.hidden = true;
    for (const file of filesInput.files ?? []) void upload(file);
    filesInput.value = "";
  });

  done.addEventListener("click", () => {
    if (activeUploads > 0) return;
    sessionStorage.removeItem(sessionKey);
    picker.hidden = true;
    done.hidden = true;
    connection.textContent = "Transfer complete";
    document.querySelector("#title").textContent = "You’re all set";
    document.querySelector("#intro").textContent = "Your files are available on the other device. You can close this page.";
  });

  claim()
    .then(() => {
      connection.textContent = "Connected securely";
      filesInput.disabled = false;
    })
    .catch((cause) => showError(cause instanceof Error ? cause.message : "Could not connect."));
})();
