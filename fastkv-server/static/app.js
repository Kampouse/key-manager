// NEAR Garden — Explorer
// Vanilla JS, no build step

const API = "";
const EXPLORER_URL = "https://nearblocks.io/txns";

function esc(s) {
  const d = document.createElement("div");
  d.textContent = s;
  return d.innerHTML;
}

function tryFormatJson(v) {
  if (typeof v !== "string") return JSON.stringify(v, null, 2);
  try {
    return JSON.stringify(JSON.parse(v), null, 2);
  } catch {
    return v;
  }
}

function buildUrl(path, params) {
  const base = API || location.origin;
  const url = new URL(path, base);
  for (const [k, v] of Object.entries(params)) {
    if (v !== undefined && v !== null) url.searchParams.set(k, String(v));
  }
  return url.toString();
}

function shQuote(s) {
  return "'" + String(s).replace(/'/g, "'\"'\"'") + "'";
}

function curlCmd(url) {
  return `curl -s ${shQuote(url)} | jq`;
}

async function copyText(text, btn) {
  try {
    await navigator.clipboard.writeText(text);
    if (btn) {
      const orig = btn.textContent;
      btn.textContent = "copied!";
      setTimeout(() => {
        btn.textContent = orig;
      }, 1500);
    }
  } catch {
    prompt("Copy:", text);
  }
}

// ── Hash state ─────────────────────────────────────────────

let hashPushing = false;

function buildHash() {
  const p = new URLSearchParams();
  if (viewMode === "write") {
    p.set("view", "write");
    if (contractId) p.set("contract", contractId);
    const keyEl = document.getElementById("write-key");
    if (keyEl && keyEl.value) p.set("key", keyEl.value);
    const valEl = document.getElementById("write-value");
    if (valEl && valEl.value) p.set("value", valEl.value);
  } else {
    if (contractId) p.set("contract", contractId);
    if (currentAccount) p.set("account", currentAccount);
    if (groupBy !== "account") p.set("groupBy", groupBy);
    const q = (queryInput.value || "").replace(/\/?\*+$/, "");
    if (q) p.set("key", q);
  }
  return p.toString();
}

function pushHash() {
  const h = buildHash();
  if (location.hash.slice(1) !== h) {
    hashPushing = true;
    location.hash = h || "";
    hashPushing = false;
  }
}

function readHash() {
  const raw = location.hash.slice(1);
  if (!raw) return false;
  const p = new URLSearchParams(raw);

  const c = p.get("contract") || "";
  contractId = c;
  contractInput.value = c;

  const view = p.get("view") || "tree";
  const key = p.get("key") || "";

  if (view === "write") {
    if (key) {
      const keyEl = document.getElementById("write-key");
      if (keyEl) keyEl.value = key;
    }
    const val = p.get("value");
    if (val != null) {
      const valEl = document.getElementById("write-value");
      if (valEl) valEl.value = val;
    }
    setViewMode("write");
    return true;
  }

  const acct = p.get("account") || "";
  currentAccount = acct;
  accountInput.value = acct;
  multiAccountMode = !acct;

  const gb = p.get("groupBy");
  if (gb === "contract") {
    groupBy = "contract";
    swapFieldOrder();
  }

  if (key) {
    queryInput.value = key + "/**";
    breadcrumb = currentAccount
      ? [currentAccount, ...key.split("/")]
      : key.split("/");
  } else {
    queryInput.value = "";
    breadcrumb = currentAccount ? [currentAccount] : [];
  }

  explore(key ? key + "/**" : "");
  return true;
}

// ── API Client ──────────────────────────────────────────────

async function kvContracts(opts) {
  const params = {};
  if (opts?.accountId) params.accountId = opts.accountId;
  if (opts?.limit) params.limit = String(opts.limit);
  if (opts?.after_contract) params.after_contract = opts.after_contract;
  const url = buildUrl("/v1/kv/contracts", params);
  const res = await fetch(url);
  if (!res.ok) throw new Error(`kvContracts: ${res.status}`);
  const json = await res.json();
  return { contracts: json.data || [], meta: json.meta || {} };
}

async function kvAccounts(contractId, opts) {
  const params = {};
  if (contractId) params.contractId = contractId;
  if (opts?.limit) params.limit = String(opts.limit);
  if (opts?.offset != null) params.offset = String(opts.offset);
  if (opts?.after_account) params.after_account = opts.after_account;
  const url = buildUrl("/v1/kv/accounts", params);
  const res = await fetch(url);
  if (!res.ok) throw new Error(`kvAccounts: ${res.status}`);
  const json = await res.json();
  return { accounts: json.data || [], meta: json.meta || {} };
}

async function kvGet(accountId, contractId, key) {
  const params = new URLSearchParams({
    accountId,
    contractId,
    key,
    value_format: "json",
  });
  const url = `${API}/v1/kv/get?${params}`;
  const res = await fetch(url);
  if (!res.ok) throw new Error(`kvGet: ${res.status}`);
  const json = await res.json();
  return json.data; // KvEntry | null
}

async function kvQueryTree(accountId, contractId, keyPrefix) {
  const params = new URLSearchParams({
    accountId,
    contractId,
    format: "tree",
    value_format: "json",
    exclude_deleted: "true",
    limit: "1000",
  });
  if (keyPrefix) params.set("key_prefix", keyPrefix);
  const url = `${API}/v1/kv/query?${params}`;
  const res = await fetch(url);
  if (!res.ok) throw new Error(`kvQueryTree: ${res.status}`);
  const json = await res.json();
  const tree = json.tree ?? json.data ?? null;
  if (json.has_more) lastTreeTruncated = true;
  return tree;
}

// ── State ───────────────────────────────────────────────────

let currentAccount = "";
let contractId = "";
let viewMode = "tree"; // 'tree' | 'write'
let treeData = null;
let lastKeyPrefix = undefined;
let lastTreeTruncated = false;
let rawData = null;
let breadcrumb = [];
let loading = false;
let multiAccountMode = true;
let currentSelectedPath = null;

// ── DOM refs ────────────────────────────────────────────────

const $ = (sel) => document.querySelector(sel);
const contractInput = $("#contract-input");
const accountInput = $("#account-input");
const queryInput = $("#query-input");
const exploreBtn = $("#explore-btn");
const exploreForm = $("#explore-form");
const breadcrumbEl = $("#breadcrumb");
const errorBar = $("#error-bar");
const errorMsg = $("#error-msg");
const retryBtn = $("#retry-btn");
const contentEl = $("#content");
const treePanel = $("#tree-panel");
const treeEl = $("#tree");
const detailPanel = $("#detail-panel");
const detailPath = $("#detail-path");
const detailValue = $("#detail-value");
const detailMeta = $("#detail-meta");
const swapBtn = $("#swap-btn");
const rowContract = $("#row-contract");
const rowAccount = $("#row-account");
let groupBy = "account"; // 'account' = account on top, 'contract' = contract on top

// ── Explorer ────────────────────────────────────────────────

const MULTI_ACCOUNT_CAP = 200;

async function explore(keyPath) {
  if (viewMode !== "tree") setViewMode("tree");
  contractId = contractInput.value.trim();
  currentAccount = accountInput.value.trim();
  multiAccountMode = !currentAccount;

  loading = true;
  exploreBtn.disabled = true;
  exploreBtn.textContent = "...";
  hideError();
  hideDetail();
  treeEl.innerHTML = '<div class="tree-loading">loading...</div>';

  // Browse mode: both fields empty — use groupBy to decide hierarchy
  if (!contractId && !currentAccount) {
    try {
      if (groupBy === "contract") {
        const { contracts } = await kvContracts({ limit: MULTI_ACCOUNT_CAP });
        if (contracts.length === 0) {
          treeData = null;
          rawData = null;
        } else {
          const placeholder = {};
          contracts.forEach((c) => {
            placeholder[c] = {};
          });
          treeData = placeholder;
          rawData = placeholder;
        }
      } else {
        const { accounts } = await kvAccounts("", { limit: MULTI_ACCOUNT_CAP });
        if (accounts.length === 0) {
          treeData = null;
          rawData = null;
        } else {
          const placeholder = {};
          accounts.forEach((a) => {
            placeholder[a] = {};
          });
          treeData = placeholder;
          rawData = placeholder;
        }
      }
    } catch (e) {
      showError("Failed to fetch data");
      console.error(e);
      treeData = null;
      rawData = null;
    }
    loading = false;
    exploreBtn.disabled = false;
    exploreBtn.textContent = "explore";
    render();
    pushHash();
    return;
  }

  // Only account filled: list contracts for that account
  if (!contractId && currentAccount) {
    try {
      const { contracts } = await kvContracts({
        accountId: currentAccount,
        limit: MULTI_ACCOUNT_CAP,
      });
      if (contracts.length === 0) {
        treeData = null;
        rawData = null;
      } else {
        const placeholder = {};
        contracts.forEach((c) => {
          placeholder[c] = {};
        });
        treeData = placeholder;
        rawData = placeholder;
      }
    } catch (e) {
      showError("Failed to fetch contracts");
      console.error(e);
      treeData = null;
      rawData = null;
    }
    loading = false;
    exploreBtn.disabled = false;
    exploreBtn.textContent = "explore";
    render();
    pushHash();
    return;
  }

  const keyPrefix =
    (keyPath || queryInput.value || "").replace(/\/?\*+$/, "") || undefined;
  lastKeyPrefix = keyPrefix;
  lastTreeTruncated = false;

  try {
    if (!multiAccountMode && currentAccount) {
      // ── Single-account: KV only ──
      const tree = await kvQueryTree(currentAccount, contractId, keyPrefix);
      if (tree && Object.keys(tree).length > 0) {
        treeData = { [currentAccount]: tree };
        rawData = treeData;
      } else {
        treeData = null;
        rawData = null;
      }
    } else {
      // ── All accounts: fetch account list, lazy-load data on expand ──
      const { accounts } = await kvAccounts(contractId, {
        limit: MULTI_ACCOUNT_CAP,
      });
      if (accounts.length === 0) {
        showError("No accounts found for this contract");
        treeData = null;
        rawData = null;
      } else {
        // Create placeholder entries — tree data loads on expand
        const placeholder = {};
        accounts.forEach((acct) => {
          placeholder[acct] = {};
        });
        treeData = placeholder;
        rawData = placeholder;
        if (accounts.length >= MULTI_ACCOUNT_CAP) {
          showError(
            `Showing first ${MULTI_ACCOUNT_CAP} accounts — more may exist`,
          );
        }
      }
    }
  } catch (e) {
    showError("Failed to fetch data");
    console.error(e);
  }

  loading = false;
  exploreBtn.disabled = false;
  exploreBtn.textContent = "explore";

  render();

  pushHash();
}

function showError(msg) {
  errorMsg.textContent = msg;
  errorBar.hidden = false;
}

function hideError() {
  errorBar.hidden = true;
}

// ── Breadcrumb ──────────────────────────────────────────────

function renderBreadcrumb() {
  breadcrumbEl.innerHTML = "";
  const segments = breadcrumb.filter(Boolean);

  if (segments.length === 0) {
    breadcrumbEl.hidden = true;
    return;
  }
  breadcrumbEl.hidden = false;

  const root = document.createElement("button");
  root.textContent = "~";
  root.type = "button";
  root.onclick = () => navigateBreadcrumb([]);
  breadcrumbEl.appendChild(root);

  segments.forEach((seg, i) => {
    const sep = document.createElement("span");
    sep.className = "sep";
    sep.textContent = "/";
    breadcrumbEl.appendChild(sep);

    if (i < breadcrumb.length - 1) {
      const btn = document.createElement("button");
      btn.textContent = seg;
      btn.type = "button";
      btn.onclick = () => navigateBreadcrumb(breadcrumb.slice(0, i + 1));
      breadcrumbEl.appendChild(btn);
    } else {
      const span = document.createElement("span");
      span.className = "current";
      span.textContent = seg;
      breadcrumbEl.appendChild(span);
    }
  });
}

function navigateBreadcrumb(segments) {
  if (segments.length === 0) {
    queryInput.value = "";
    breadcrumb = [];
    explore();
  } else {
    setAccount(segments[0]);
    if (segments.length === 1) {
      queryInput.value = "";
      breadcrumb = [currentAccount];
      explore();
    } else {
      const keyPath = segments.slice(1).join("/");
      queryInput.value = `${keyPath}/**`;
      breadcrumb = segments;
      explore(`${keyPath}/**`);
    }
  }
}

// ── Tree rendering ──────────────────────────────────────────

function renderEmptyState() {
  const prefix = lastKeyPrefix;
  const el = document.createElement("div");
  el.className = "tree-empty";

  let heading = "";
  let detail = "";
  if (!contractId) {
    const acct = accountInput.value.trim();
    heading = acct
      ? `No contracts found for ${esc(acct)}`
      : "No contracts found";
  } else if (prefix) {
    heading = `No entries under "${esc(prefix)}/"`;
    detail = currentAccount
      ? `for ${esc(currentAccount)} on ${esc(contractId)}`
      : `on ${esc(contractId)}`;
  } else {
    heading = "No data found";
    detail = currentAccount
      ? `${esc(currentAccount)} has no entries on ${esc(contractId)}`
      : `no entries on ${esc(contractId)}`;
  }

  let html = `<div class="empty-heading">${heading}</div>`;
  if (detail) html += `<div class="empty-detail">${detail}</div>`;

  el.innerHTML = html;
  treeEl.appendChild(el);
}

function renderTree() {
  treeEl.innerHTML = "";

  if (loading && !treeData) {
    treeEl.innerHTML = '<div class="tree-loading">loading...</div>';
    return;
  }

  if (!treeData) {
    renderEmptyState();
    return;
  }

  if (!contractId && !currentAccount) {
    // Browse all: groupBy determines hierarchy
    const keys = Object.keys(treeData);
    if (keys.length === 0) {
      renderEmptyState();
      return;
    }
    if (groupBy === "contract") {
      keys.forEach((c) => {
        treeEl.appendChild(createContractNode(c));
      });
    } else {
      keys.forEach((a) => {
        treeEl.appendChild(createAccountNode(a));
      });
    }
    return;
  }

  if (!contractId && currentAccount) {
    // Account filled, no contract: show contracts for this account
    const contracts = Object.keys(treeData);
    if (contracts.length === 0) {
      renderEmptyState();
      return;
    }
    contracts.forEach((c) => {
      treeEl.appendChild(createContractUnderAccountNode(c, currentAccount));
    });
    return;
  }

  if (multiAccountMode) {
    // All-accounts mode: show accounts as expandable nodes (lazy-loaded)
    const accounts = Object.entries(treeData);
    if (accounts.length === 0) {
      renderEmptyState();
      return;
    }
    accounts.forEach(([acct, val]) => {
      treeEl.appendChild(createTreeNode(acct, val, "", 0, acct));
    });
    return;
  }

  // Single-account mode: flatten top-level account wrapper
  const entries = Object.entries(treeData).flatMap(([_acct, val]) =>
    typeof val === "object" && val !== null ? Object.entries(val) : [],
  );

  if (entries.length === 0) {
    renderEmptyState();
    return;
  }

  entries.forEach(([key, val]) => {
    treeEl.appendChild(createTreeNode(key, val, key, 0));
  });

  if (lastTreeTruncated) {
    const note = document.createElement("div");
    note.className = "tree-truncated";
    note.textContent =
      "Results truncated — narrow your query or use the API for full results";
    treeEl.appendChild(note);
  }
}

function createTreeNode(
  name,
  value,
  path,
  depth,
  accountOverride,
  contractOverride,
) {
  const container = document.createElement("div");
  const isBranch = typeof value === "object" && value !== null;
  const isNearAccount = name.endsWith(".near") || name.endsWith(".tg");
  let expanded = false;
  let childrenLoaded = isBranch && Object.keys(value).length > 0;
  let children = isBranch ? value : null;

  // The clickable row
  const row = document.createElement("div");
  row.className = "tree-item";
  row.tabIndex = 0;
  row.setAttribute("role", "treeitem");

  // Icon
  const icon = document.createElement("span");
  icon.className = "tree-icon" + (isBranch ? "" : " leaf");
  icon.textContent = isBranch ? "\u25b6" : "=";
  row.appendChild(icon);

  // Name
  const nameEl = document.createElement("span");
  nameEl.className =
    "tree-name" +
    (isBranch ? " branch" : "") +
    (isNearAccount ? " near-account" : "");
  nameEl.textContent = name;
  if (isNearAccount) {
    nameEl.onclick = (e) => {
      e.stopPropagation();
      navigateToAccount(name);
    };
  }
  row.appendChild(nameEl);

  // Leaf value preview
  if (!isBranch && value !== null && value !== undefined) {
    const preview = document.createElement("span");
    preview.className = "tree-preview";
    const str = typeof value === "string" ? value : JSON.stringify(value);
    preview.textContent = str.length > 60 ? str.slice(0, 60) + "..." : str;
    row.appendChild(preview);
  }

  container.appendChild(row);

  // Children container
  const childrenEl = document.createElement("div");
  childrenEl.className = "tree-children";
  childrenEl.hidden = true;
  container.appendChild(childrenEl);

  function toggle() {
    if (!isBranch) {
      selectNode(path, value, accountOverride, contractOverride);
      return;
    }
    expanded = !expanded;
    icon.textContent = expanded ? "\u25bc" : "\u25b6";
    row.setAttribute("aria-expanded", expanded);

    if (expanded && !childrenLoaded) {
      loadChildren();
    } else {
      childrenEl.hidden = !expanded;
    }
  }

  async function loadChildren() {
    icon.textContent = "...";
    try {
      const tree = await kvQueryTree(
        accountOverride || currentAccount,
        contractOverride || contractId,
        path || undefined,
      );
      children = tree && typeof tree === "object" ? tree : {};
      childrenLoaded = true;
      renderChildren();
      childrenEl.hidden = false;
    } catch (e) {
      console.error(`Failed to load children for ${path}:`, e);
      childrenEl.innerHTML = '<div class="tree-empty">failed_</div>';
      childrenEl.hidden = false;
    }
    icon.textContent = expanded ? "\u25bc" : "\u25b6";
  }

  function renderChildren() {
    childrenEl.innerHTML = "";
    const entries = Object.entries(children);
    if (entries.length === 0) {
      childrenEl.innerHTML = '<div class="tree-empty">(empty)</div>';
      return;
    }
    entries.forEach(([k, v]) => {
      childrenEl.appendChild(
        createTreeNode(
          k,
          v,
          `${path}/${k}`,
          depth + 1,
          accountOverride,
          contractOverride,
        ),
      );
    });
  }

  // If branch already has children data, pre-render them
  if (isBranch && childrenLoaded) {
    renderChildren();
  }

  row.onclick = toggle;
  row.onkeydown = (e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      toggle();
    }
  };

  return container;
}

function createAccountNode(accountName) {
  const container = document.createElement("div");
  let expanded = false;
  let childrenLoaded = false;

  const row = document.createElement("div");
  row.className = "tree-item";
  row.tabIndex = 0;
  row.setAttribute("role", "treeitem");

  const icon = document.createElement("span");
  icon.className = "tree-icon";
  icon.textContent = "\u25b6";
  row.appendChild(icon);

  const nameEl = document.createElement("span");
  nameEl.className = "tree-name branch near-account";
  nameEl.textContent = accountName;
  nameEl.onclick = (e) => {
    e.stopPropagation();
    navigateToAccount(accountName);
  };
  row.appendChild(nameEl);

  container.appendChild(row);

  const childrenEl = document.createElement("div");
  childrenEl.className = "tree-children";
  childrenEl.hidden = true;
  container.appendChild(childrenEl);

  async function toggle() {
    expanded = !expanded;
    icon.textContent = expanded ? "\u25bc" : "\u25b6";
    row.setAttribute("aria-expanded", expanded);

    if (expanded && !childrenLoaded) {
      icon.textContent = "...";
      try {
        const { contracts } = await kvContracts({
          accountId: accountName,
          limit: MULTI_ACCOUNT_CAP,
        });
        childrenLoaded = true;
        childrenEl.innerHTML = "";
        if (contracts.length === 0) {
          childrenEl.innerHTML = '<div class="tree-empty">(no contracts)</div>';
        } else {
          contracts.forEach((c) => {
            childrenEl.appendChild(
              createContractUnderAccountNode(c, accountName),
            );
          });
        }
        childrenEl.hidden = false;
      } catch (e) {
        console.error(`Failed to load contracts for ${accountName}:`, e);
        childrenEl.innerHTML = '<div class="tree-empty">failed_</div>';
        childrenEl.hidden = false;
      }
      icon.textContent = expanded ? "\u25bc" : "\u25b6";
    } else {
      childrenEl.hidden = !expanded;
    }
  }

  row.onclick = toggle;
  row.onkeydown = (e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      toggle();
    }
  };

  return container;
}

function createContractUnderAccountNode(contractName, parentAccount) {
  const container = document.createElement("div");
  let expanded = false;
  let childrenLoaded = false;

  const row = document.createElement("div");
  row.className = "tree-item";
  row.tabIndex = 0;
  row.setAttribute("role", "treeitem");

  const icon = document.createElement("span");
  icon.className = "tree-icon";
  icon.textContent = "\u25b6";
  row.appendChild(icon);

  const nameEl = document.createElement("span");
  nameEl.className = "tree-name branch";
  nameEl.textContent = contractName;
  nameEl.onclick = (e) => {
    e.stopPropagation();
    contractInput.value = contractName;
    contractId = contractName;
    navigateToAccount(parentAccount);
  };
  row.appendChild(nameEl);

  container.appendChild(row);

  const childrenEl = document.createElement("div");
  childrenEl.className = "tree-children";
  childrenEl.hidden = true;
  container.appendChild(childrenEl);

  async function toggle() {
    expanded = !expanded;
    icon.textContent = expanded ? "\u25bc" : "\u25b6";
    row.setAttribute("aria-expanded", expanded);

    if (expanded && !childrenLoaded) {
      icon.textContent = "...";
      try {
        const tree = await kvQueryTree(parentAccount, contractName);
        childrenLoaded = true;
        childrenEl.innerHTML = "";
        const entries =
          tree && typeof tree === "object" ? Object.entries(tree) : [];
        if (entries.length === 0) {
          childrenEl.innerHTML = '<div class="tree-empty">(empty)</div>';
        } else {
          entries.forEach(([key, val]) => {
            childrenEl.appendChild(
              createTreeNode(key, val, key, 0, parentAccount, contractName),
            );
          });
        }
        childrenEl.hidden = false;
      } catch (e) {
        console.error(
          `Failed to load data for ${parentAccount}/${contractName}:`,
          e,
        );
        childrenEl.innerHTML = '<div class="tree-empty">failed_</div>';
        childrenEl.hidden = false;
      }
      icon.textContent = expanded ? "\u25bc" : "\u25b6";
    } else {
      childrenEl.hidden = !expanded;
    }
  }

  row.onclick = toggle;
  row.onkeydown = (e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      toggle();
    }
  };

  return container;
}

function createContractNode(contractName) {
  const container = document.createElement("div");
  let expanded = false;
  let childrenLoaded = false;

  const row = document.createElement("div");
  row.className = "tree-item";
  row.tabIndex = 0;
  row.setAttribute("role", "treeitem");

  const icon = document.createElement("span");
  icon.className = "tree-icon";
  icon.textContent = "\u25b6";
  row.appendChild(icon);

  const nameEl = document.createElement("span");
  nameEl.className = "tree-name branch";
  nameEl.textContent = contractName;
  nameEl.onclick = (e) => {
    e.stopPropagation();
    contractInput.value = contractName;
    contractId = contractName;
    multiAccountMode = true;
    accountInput.value = "";
    currentAccount = "";
    queryInput.value = "";
    breadcrumb = [];
    explore();
  };
  row.appendChild(nameEl);

  container.appendChild(row);

  const childrenEl = document.createElement("div");
  childrenEl.className = "tree-children";
  childrenEl.hidden = true;
  container.appendChild(childrenEl);

  async function toggle() {
    expanded = !expanded;
    icon.textContent = expanded ? "\u25bc" : "\u25b6";
    row.setAttribute("aria-expanded", expanded);

    if (expanded && !childrenLoaded) {
      icon.textContent = "...";
      try {
        const { accounts } = await kvAccounts(contractName, {
          limit: MULTI_ACCOUNT_CAP,
        });
        childrenLoaded = true;
        childrenEl.innerHTML = "";
        if (accounts.length === 0) {
          childrenEl.innerHTML = '<div class="tree-empty">(no accounts)</div>';
        } else {
          accounts.forEach((acct) => {
            childrenEl.appendChild(
              createAccountUnderContractNode(acct, contractName),
            );
          });
        }
        childrenEl.hidden = false;
      } catch (e) {
        console.error(`Failed to load accounts for ${contractName}:`, e);
        childrenEl.innerHTML = '<div class="tree-empty">failed_</div>';
        childrenEl.hidden = false;
      }
      icon.textContent = expanded ? "\u25bc" : "\u25b6";
    } else {
      childrenEl.hidden = !expanded;
    }
  }

  row.onclick = toggle;
  row.onkeydown = (e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      toggle();
    }
  };

  return container;
}

function createAccountUnderContractNode(accountName, parentContract) {
  const container = document.createElement("div");
  let expanded = false;
  let childrenLoaded = false;

  const row = document.createElement("div");
  row.className = "tree-item";
  row.tabIndex = 0;
  row.setAttribute("role", "treeitem");

  const icon = document.createElement("span");
  icon.className = "tree-icon";
  icon.textContent = "\u25b6";
  row.appendChild(icon);

  const nameEl = document.createElement("span");
  nameEl.className = "tree-name branch near-account";
  nameEl.textContent = accountName;
  nameEl.onclick = (e) => {
    e.stopPropagation();
    contractInput.value = parentContract;
    contractId = parentContract;
    navigateToAccount(accountName);
  };
  row.appendChild(nameEl);

  container.appendChild(row);

  const childrenEl = document.createElement("div");
  childrenEl.className = "tree-children";
  childrenEl.hidden = true;
  container.appendChild(childrenEl);

  async function toggle() {
    expanded = !expanded;
    icon.textContent = expanded ? "\u25bc" : "\u25b6";
    row.setAttribute("aria-expanded", expanded);

    if (expanded && !childrenLoaded) {
      icon.textContent = "...";
      try {
        const tree = await kvQueryTree(accountName, parentContract);
        childrenLoaded = true;
        childrenEl.innerHTML = "";
        const entries =
          tree && typeof tree === "object" ? Object.entries(tree) : [];
        if (entries.length === 0) {
          childrenEl.innerHTML = '<div class="tree-empty">(empty)</div>';
        } else {
          entries.forEach(([key, val]) => {
            childrenEl.appendChild(
              createTreeNode(key, val, key, 0, accountName, parentContract),
            );
          });
        }
        childrenEl.hidden = false;
      } catch (e) {
        console.error(
          `Failed to load keys for ${accountName} on ${parentContract}:`,
          e,
        );
        childrenEl.innerHTML = '<div class="tree-empty">failed_</div>';
        childrenEl.hidden = false;
      }
      icon.textContent = expanded ? "\u25bc" : "\u25b6";
    } else {
      childrenEl.hidden = !expanded;
    }
  }

  row.onclick = toggle;
  row.onkeydown = (e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      toggle();
    }
  };

  return container;
}

// ── Detail panel ────────────────────────────────────────────

function selectNode(path, value, accountOverride, contractOverride) {
  const acct = accountOverride || currentAccount;
  const cid = contractOverride || contractId;
  currentSelectedPath = path;
  detailPanel.hidden = false;

  // Path + copy buttons
  detailPath.innerHTML = "";
  const pathText = document.createElement("span");
  pathText.textContent = `${acct}/${path}`;
  detailPath.appendChild(pathText);

  const getUrl = buildUrl("/v1/kv/get", {
    accountId: acct,
    contractId: cid,
    key: path,
    value_format: "json",
  });
  const copyBar = document.createElement("div");
  copyBar.className = "copy-bar";
  const cpUrl = document.createElement("button");
  cpUrl.className = "copy-btn";
  cpUrl.textContent = "copy url";
  cpUrl.onclick = () => copyText(getUrl, cpUrl);
  const cpCurl = document.createElement("button");
  cpCurl.className = "copy-btn";
  cpCurl.textContent = "copy curl";
  cpCurl.onclick = () => copyText(curlCmd(getUrl), cpCurl);
  copyBar.appendChild(cpUrl);
  copyBar.appendChild(cpCurl);
  detailPath.appendChild(copyBar);

  // Placeholder value from tree
  detailValue.textContent = tryFormatJson(value);
  detailMeta.innerHTML = '<div class="tree-loading">loading_</div>';

  kvGet(acct, cid, path)
    .then((entry) => {
      if (entry) {
        detailValue.textContent = tryFormatJson(entry.value);
        renderDetailMeta(entry);
      } else {
        detailMeta.innerHTML = "";
      }
    })
    .catch((e) => {
      console.error(`Failed to load detail for ${path}:`, e);
      detailMeta.innerHTML = '<div class="tree-empty">failed_</div>';
    });
}

function renderDetailMeta(entry) {
  let html = "";
  if (entry.blockHeight != null) {
    html += `<div><span class="meta-label">block: </span><span class="meta-value">${esc(String(entry.blockHeight))}</span></div>`;
  }
  if (entry.txHash) {
    html += `<div><span class="meta-label">tx: </span><a href="${EXPLORER_URL}/${encodeURIComponent(entry.txHash)}" target="_blank" rel="noopener noreferrer">${esc(entry.txHash.slice(0, 12))}...</a></div>`;
  }
  if (entry.receiptId) {
    html += `<div><span class="meta-label">receipt: </span><span class="meta-value">${esc(entry.receiptId.slice(0, 12))}...</span></div>`;
  }
  if (entry.accountId) {
    html += `<div><span class="meta-label">writer: </span><span class="meta-writer">${esc(entry.accountId)}</span></div>`;
  }
  if (entry.isDeleted) {
    html += `<div><span class="meta-label">deleted: </span><span style="color:var(--danger)">true</span></div>`;
  }
  detailMeta.innerHTML = html;
}

function hideDetail() {
  detailPanel.hidden = true;
  currentSelectedPath = null;
}

// ── Navigation ──────────────────────────────────────────────

function navigateToAccount(accountId) {
  setAccount(accountId);
  queryInput.value = "";
  breadcrumb = [currentAccount];
  explore();
}

// ── View mode ───────────────────────────────────────────────

const writePanel = $("#write-panel");

function setViewMode(mode) {
  viewMode = mode;
  render();
  pushHash();
}

// ── Render ──────────────────────────────────────────────────

function render() {
  renderBreadcrumb();

  // Hide all panels first
  contentEl.hidden = true;
  if (writePanel) writePanel.hidden = true;

  if (viewMode === "write") {
    if (writePanel) writePanel.hidden = false;
  } else {
    contentEl.hidden = false;
    renderTree();
  }
}

// ── Event listeners ─────────────────────────────────────────

document.getElementById("home-link")?.addEventListener("click", (e) => {
  e.preventDefault();
  contractId = "";
  contractInput.value = "";
  currentAccount = "";
  accountInput.value = "";
  queryInput.value = "";
  breadcrumb = [];
  multiAccountMode = true;
  groupBy = "account";
  setViewMode("tree");
  explore();
});

exploreForm.onsubmit = (e) => {
  e.preventDefault();
  const q = queryInput.value.trim();
  currentAccount = accountInput.value.trim();
  if (q) {
    const keyParts = q.replace(/\/?\*+$/, "").split("/");
    breadcrumb = currentAccount ? [currentAccount, ...keyParts] : keyParts;
    explore(q);
  } else {
    breadcrumb = currentAccount ? [currentAccount] : [];
    explore();
  }
};

retryBtn.onclick = () => explore();

contractInput.onchange = () => {
  contractId = contractInput.value;
  pushHash();
};

accountInput.onchange = () => {
  currentAccount = accountInput.value;
};

function swapFieldOrder() {
  const parent = rowContract.parentElement;
  if (groupBy === "account") {
    parent.insertBefore(rowAccount, rowContract);
  } else {
    parent.insertBefore(rowContract, rowAccount);
  }
  // Keep swap button between the two rows
  parent.insertBefore(swapBtn, parent.children[1]);
}

if (swapBtn) {
  swapBtn.onclick = () => {
    groupBy = groupBy === "contract" ? "account" : "contract";
    swapFieldOrder();
  };
}

function setAccount(accountId) {
  currentAccount = accountId;
  multiAccountMode = false;
  if (accountInput) accountInput.value = accountId;
}

// ── Init ────────────────────────────────────────────────────

window.addEventListener("hashchange", () => {
  if (hashPushing) return;
  readHash();
});

swapFieldOrder();
currentAccount = accountInput.value.trim();
breadcrumb = currentAccount ? [currentAccount] : [];
if (!readHash()) explore();
