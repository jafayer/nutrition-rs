function fmt(amount, unit) {
  const a = Math.abs(amount - Math.round(amount)) < 0.005 ? Math.round(amount) : amount.toFixed(1);
  return unit ? `${a} ${unit}` : `${a}`;
}

function parseQuantityStr(s) {
  s = (s || '').trim();
  const m = s.match(/^([0-9]+(?:\.[0-9]+)?)\s*(.*)$/);
  if (!m) return null;
  return { amount: parseFloat(m[1]), unit: m[2].trim() || null };
}

function addRemoveHandlers() {
  document.querySelectorAll('.remove-btn').forEach(btn => {
    btn.onclick = () => {
      const list = btn.closest('.repeatable-list');
      if (list.querySelectorAll('.repeatable-item').length > 1) btn.closest('.repeatable-item').remove();
    };
  });
}

// Client-side cache for aliases to avoid repeated API calls
const aliasCache = {};

// Fetch aliases from API and populate datalist elements
async function fetchAndPopulateAliases(datalistId, endpoint) {
  const datalist = document.getElementById(datalistId);
  if (!datalist) return;

  // Check cache first
  if (aliasCache[endpoint]) {
    populateDatalist(datalist, aliasCache[endpoint]);
    return;
  }

  try {
    const response = await fetch(endpoint);
    if (!response.ok) return;
    const aliases = await response.json();
    aliasCache[endpoint] = aliases;
    populateDatalist(datalist, aliases);
  } catch (err) {
    console.error(`Failed to fetch aliases from ${endpoint}:`, err);
  }
}

function populateDatalist(datalist, aliases) {
  datalist.innerHTML = aliases.map(alias => `<option value="${alias}">`).join('');
}