'use strict';
const prefix = document.querySelector('meta[name="stargate-prefix"]').content;
const el = id => document.getElementById(id);
let csrf, identity;
function notice(message) { el('notice').textContent = message; }
async function api(path, method = 'GET', body) {
  const response = await fetch(prefix + path, {method, credentials: 'same-origin', headers: {'content-type': 'application/json', ...(csrf ? {'x-stargate-csrf': csrf} : {})}, ...(body ? {body: JSON.stringify(body)} : {})});
  if (!response.ok) { if (response.status === 401) { showLogin(); } throw new Error(response.status === 403 ? 'Access denied. Reload the page and try again.' : 'The request could not be completed.'); }
  return response.status === 204 ? null : response.json();
}
function show(id) { for (const section of ['login','profile','keys','sessions']) el(section).hidden = section !== id; }
async function showLogin() {
  csrf = null; identity = null; show('login'); el('logout').hidden = true;
  const config = await api('/api/config'); el('providers').replaceChildren();
  for (const provider of config.providers) { const a = document.createElement('a'); a.className = 'provider'; a.href = `${prefix}/login?provider=${encodeURIComponent(provider)}&return_to=${prefix}/profile`; a.textContent = `Continue with ${provider}`; el('providers').append(a); }
  if (!config.providers.length) notice('No sign-in provider is configured.');
}
function row(list, title, detail, action, disabled = false) {
  const li = document.createElement('li'), text = document.createElement('span'), strong = document.createElement('strong'), small = document.createElement('small');
  strong.textContent = title; small.textContent = detail; text.append(strong, document.createElement('br'), small); li.append(text);
  const button = document.createElement('button'); button.textContent = disabled ? 'Revoked' : 'Revoke'; button.disabled = disabled;
  button.onclick = async () => { if (!confirm(`Revoke ${title}?`)) return; button.disabled = true; try { await action(); } catch (e) { button.disabled = false; notice(e.message); } }; li.append(button); el(list).append(li);
}
async function keys() {
  const data = await api('/api/keys'); el('key-list').replaceChildren();
  for (const key of data) row('key-list', key.name, `${key.prefix}… · ${key.scopes.join(', ') || 'No scopes'}${key.expires_at ? ' · Expires '+new Date(key.expires_at*1000).toLocaleString() : ''}`, async () => { await api('/api/keys/'+key.id,'DELETE'); await keys(); }, key.revoked_at != null);
  if (!data.length) el('key-list').textContent = 'No API keys yet.';
}
async function sessions(current) {
  const data = await api('/api/sessions'); el('session-list').replaceChildren();
  for (const s of data) row('session-list', s.id === current ? 'Current session' : 'Session', `Created ${new Date(s.created_at*1000).toLocaleString()} · Expires ${new Date(s.expires_at*1000).toLocaleString()}`, async () => { await api('/api/sessions/'+s.id,'DELETE'); if (s.id === current) await showLogin(); else await sessions(current); }, s.revoked_at != null);
}
el('create-key').onsubmit = async event => {
  event.preventDefault(); const form = event.target, button = form.querySelector('button'); button.disabled = true;
  try { const key = await api('/api/keys','POST',{name:form.elements.name.value,scopes:form.elements.scopes.value.trim().split(/\s+/).filter(Boolean),expires_at:form.elements.expires.value ? Math.floor(new Date(form.elements.expires.value).getTime()/1000) : null}); el('secret').textContent = key.secret; el('new-secret').hidden = false; form.reset(); await keys(); notice('Key created. Copy the secret before closing this page.'); } catch (e) { notice(e.message); } finally { button.disabled = false; }
};
el('copy-secret').onclick = async () => { try { await navigator.clipboard.writeText(el('secret').textContent); notice('Key copied.'); } catch { notice('Select and copy the key manually.'); } };
el('dismiss-secret').onclick = () => { el('secret').textContent = ''; el('new-secret').hidden = true; };
el('logout').onclick = async () => { try { await api('/logout','POST'); await showLogin(); } catch (e) { notice(e.message); } };
el('revoke-all').onclick = async () => { if (!confirm('Revoke every session, including this one?')) return; try { await api('/api/sessions','DELETE'); await showLogin(); } catch (e) { notice(e.message); } };
(async () => {
  try {
    const me = await api('/api/me'); csrf = me.csrf_token; identity = me.identity; el('logout').hidden = false;
    for (const [name,value] of [['User',identity.user_id],['Email',identity.email || 'Not provided'],['Scopes',identity.scopes.join(', ') || 'None']]) { const dt=document.createElement('dt'),dd=document.createElement('dd');dt.textContent=name;dd.textContent=value;el('identity').append(dt,dd); }
    const page = location.pathname.slice(prefix.length).replace(/^\//,'') || 'profile'; show(['profile','keys','sessions'].includes(page) ? page : 'profile');
    for (const a of document.querySelectorAll('nav a')) if (a.pathname === location.pathname) a.setAttribute('aria-current','page');
    if (page === 'keys') await keys(); if (page === 'sessions') await sessions(me.session_id);
  } catch (e) { if (identity) notice(e.message); else await showLogin(); }
})();
