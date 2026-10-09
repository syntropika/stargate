import { useCallback, useEffect, useState, type FormEvent, type ReactNode } from 'react';
import { LocalAccess, PasswordChange, type LocalStatus } from './local-access';
import { UserManagement } from './user-management';

export type Props = {
  prefix: string;
  name: string;
  logo: string | null;
  providers: string[];
  page: string;
  local?: boolean;
};
type Identity = {
  user_id: string;
  email: string | null;
  scopes: string[];
  claims?: { stargate?: { role?: string } };
};
type Me = { identity: Identity; csrf_token: string; session_id: string; local_password?: boolean };
type Key = {
  id: string;
  name: string;
  prefix: string;
  scopes: string[];
  expires_at: number | null;
  revoked_at: number | null;
};
type Session = { id: string; created_at: number; expires_at: number; revoked_at: number | null };

const date = (value: number) => new Date(value * 1000).toLocaleString();
type LoadState = 'loading' | 'ready' | 'error';

class ApiError extends Error {
  constructor(readonly status: number) {
    super(
      status === 409
        ? 'Cannot complete this change. The email may already be used, or the last administrator must remain active.'
        : status === 429
          ? 'Too many attempts. Wait a few minutes and try again.'
          : status === 401
            ? 'Your session ended. Reload the page to sign in again.'
            : status === 403
              ? 'Access denied. Reload the page and try again.'
              : 'The request could not be completed.',
    );
  }
}

function PageHeading({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="page-heading">
      <h1>{title}</h1>
      <p>{children}</p>
    </div>
  );
}

function EmptyState({
  title,
  children,
  headingLevel = 3,
}: {
  title: string;
  children: ReactNode;
  headingLevel?: 2 | 3;
}) {
  const Heading = headingLevel === 2 ? 'h2' : 'h3';
  return (
    <div className="empty-state">
      <Heading>{title}</Heading>
      <p>{children}</p>
    </div>
  );
}

function LoadingState({ label }: { label: string }) {
  return (
    <div className="loading-state">
      <output>{label}</output>
      <div className="loading-lines" aria-hidden="true">
        <span />
        <span />
        <span />
      </div>
    </div>
  );
}

function ResourceRow({
  title,
  children,
  action,
}: {
  title: string;
  children: ReactNode;
  action: ReactNode;
}) {
  return (
    <li className="resource-row">
      <div className="resource-details">
        <strong>{title}</strong>
        <div className="resource-meta">{children}</div>
      </div>
      {action}
    </li>
  );
}

export function App({ prefix, name, logo, providers, page, local = false }: Props) {
  const [me, setMe] = useState<Me | null>(null);
  const [notice, setNotice] = useState('');
  const [keys, setKeys] = useState<Key[]>([]);
  const [sessions, setSessions] = useState<Session[]>([]);
  const [secret, setSecret] = useState('');
  const [busy, setBusy] = useState(false);
  const [accountState, setAccountState] = useState<LoadState>('loading');
  const [collectionState, setCollectionState] = useState<LoadState>('loading');
  const [localStatus, setLocalStatus] = useState<LocalStatus | null>(null);
  const administrator = me?.identity.claims?.stargate?.role === 'administrator';
  const view = ['profile', 'keys', 'sessions', 'users'].includes(page) ? page : 'profile';

  const api = useCallback(
    async (path: string, method = 'GET', body?: unknown) => {
      const response = await fetch(prefix + path, {
        method,
        credentials: 'same-origin',
        headers: {
          'content-type': 'application/json',
          ...(me ? { 'x-stargate-csrf': me.csrf_token } : {}),
        },
        ...(body !== undefined ? { body: JSON.stringify(body) } : {}),
      });
      if (!response.ok) {
        throw new ApiError(response.status);
      }
      return response.status === 204 ? null : response.json();
    },
    [prefix, me],
  );

  useEffect(() => {
    let active = true;
    Promise.all([
      fetch(prefix + '/api/me', { credentials: 'same-origin' }),
      local ? fetch(prefix + '/api/local', { credentials: 'same-origin' }) : Promise.resolve(null),
    ])
      .then(async ([account, configuration]) => {
        if (configuration) {
          if (!configuration.ok) throw new Error('Sign-in configuration could not be loaded.');
          const status = await configuration.json();
          if (active) setLocalStatus(status);
        }
        return account;
      })
      .then(async (response) => {
        if (response.status === 401) return null;
        if (!response.ok) throw new Error('The account could not be loaded.');
        return response.json();
      })
      .then((value) => {
        if (active) {
          setMe(value);
          setAccountState('ready');
        }
      })
      .catch((error) => {
        if (active) {
          setNotice(error.message);
          setAccountState('error');
        }
      });
    return () => {
      active = false;
    };
  }, [prefix, local]);

  useEffect(() => {
    if (!me) return;
    let active = true;
    if (view === 'keys' || view === 'sessions') {
      api('/api/' + view)
        .then((data) => {
          if (active) {
            if (view === 'keys') setKeys(data);
            else setSessions(data);
            setCollectionState('ready');
          }
        })
        .catch((error) => {
          if (active) {
            if (error instanceof ApiError && error.status === 401) {
              setMe(null);
              setSecret('');
            }
            setNotice(error.message);
            setCollectionState('error');
          }
        });
    }
    return () => {
      active = false;
    };
  }, [api, me, view]);

  async function action(run: () => Promise<void>) {
    setBusy(true);
    setNotice('');
    try {
      await run();
    } catch (error) {
      if (error instanceof ApiError && error.status === 401) {
        setMe(null);
        setSecret('');
      }
      setNotice((error as Error).message);
    } finally {
      setBusy(false);
    }
  }

  async function createKey(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    const data = new FormData(form);
    await action(async () => {
      const expires = String(data.get('expires') || '');
      const result = await api('/api/keys', 'POST', {
        name: data.get('name'),
        scopes: String(data.get('scopes') || '')
          .trim()
          .split(/\s+/)
          .filter(Boolean),
        expires_at: expires ? Math.floor(new Date(expires).getTime() / 1000) : null,
      });
      setSecret(result.secret);
      form.reset();
      setKeys(await api('/api/keys'));
      setNotice('Key created. Copy the secret before closing this page.');
    });
  }

  const authenticated = accountState === 'ready' && me !== null;

  return (
    <div className="account-app">
      <a className="skip-link" href="#account-content">
        Skip to content
      </a>
      <header className="brand-header">
        <a className="brand-lockup" href={prefix + '/'}>
          <img className="logo" src={logo || prefix + '/assets/stargate.svg'} alt="" />
          <span>{name}</span>
        </a>
        {authenticated && (
          <button
            id="logout"
            className="secondary-button sign-out"
            type="button"
            disabled={busy}
            onClick={() =>
              action(async () => {
                await api('/logout', 'POST');
                setMe(null);
                setSecret('');
                setNotice('');
              })
            }
          >
            Sign out
          </button>
        )}
      </header>
      {authenticated && (
        <nav className="account-navigation" aria-label="Account">
          {['profile', 'keys', 'sessions', ...(administrator && local ? ['users'] : [])].map(
            (item) => (
              <a
                key={item}
                href={prefix + '/' + item}
                aria-current={view === item ? 'page' : undefined}
              >
                {item === 'keys' ? 'API keys' : item[0].toUpperCase() + item.slice(1)}
              </a>
            ),
          )}
        </nav>
      )}
      <main className={authenticated ? 'account-shell' : 'account-shell anonymous-shell'}>
        <div id="account-content" className="account-content" tabIndex={-1}>
          <output id="notice" className="notice" aria-live="polite">
            {notice}
          </output>
          {accountState === 'loading' && <LoadingState label="Loading your account…" />}
          {accountState === 'error' && (
            <section className="account-view">
              <PageHeading title="Account unavailable">
                Reload the page to try loading your account again.
              </PageHeading>
              <a className="secondary-button" href={prefix + '/' + view}>
                Reload account
              </a>
            </section>
          )}
          {accountState === 'ready' && !me && (
            <section id="login" className="account-view login-view">
              <PageHeading
                title={
                  localStatus?.setup_available
                    ? 'Create administrator account'
                    : 'Your account, connected.'
                }
              >
                {localStatus?.setup_available
                  ? 'The first completed account will manage users and access. Setup closes after this account is created.'
                  : 'Sign in to manage your profile, API keys and sessions.'}
              </PageHeading>
              {localStatus?.enabled && <LocalAccess prefix={prefix} status={localStatus} />}
              <div id="providers" className="provider-list">
                {!localStatus?.setup_available &&
                  providers.map((provider) => (
                    <a
                      key={provider}
                      className="provider"
                      href={`${prefix}/login?provider=${encodeURIComponent(provider)}&return_to=${encodeURIComponent(prefix + '/profile')}`}
                    >
                      Continue with {provider}
                    </a>
                  ))}
              </div>
              {!providers.length && !local && (
                <EmptyState title="Sign-in is not configured" headingLevel={2}>
                  Contact the service administrator to enable a sign-in provider.
                </EmptyState>
              )}
            </section>
          )}
          {authenticated && view === 'profile' && (
            <section id="profile" className="account-view">
              <PageHeading title="Profile">Your account identity and access scopes.</PageHeading>
              <section className="content-section" aria-labelledby="identity-heading">
                <h2 id="identity-heading">Account details</h2>
                <dl id="identity" className="identity-list">
                  <div>
                    <dt>User</dt>
                    <dd>{me.identity.user_id}</dd>
                  </div>
                  <div>
                    <dt>Email</dt>
                    <dd>{me.identity.email || 'Not provided'}</dd>
                  </div>
                  <div>
                    <dt>Scopes</dt>
                    <dd>{me.identity.scopes.join(', ') || 'None'}</dd>
                  </div>
                </dl>
              </section>
              {me.local_password && <PasswordChange prefix={prefix} csrf={me.csrf_token} />}
            </section>
          )}
          {authenticated &&
            view === 'users' &&
            (administrator && local ? (
              <UserManagement api={api} prefix={prefix} currentUserId={me.identity.user_id} />
            ) : (
              <section className="account-view">
                <PageHeading title="Access denied">
                  Only administrators can manage users.
                </PageHeading>
                <a className="secondary-button" href={prefix + '/profile'}>
                  Return to profile
                </a>
              </section>
            ))}
          {authenticated && view === 'keys' && (
            <section id="keys" className="account-view">
              <PageHeading title="API keys">
                Create credentials for your apps. Each secret is shown once.
              </PageHeading>
              <section
                className="content-section collection-section"
                aria-labelledby="key-list-heading"
                aria-busy={collectionState === 'loading'}
              >
                <h2 id="key-list-heading">Your keys</h2>
                {collectionState === 'loading' ? (
                  <LoadingState label="Loading API keys…" />
                ) : collectionState === 'error' ? (
                  <p className="collection-error">
                    API keys could not be loaded. <a href={prefix + '/keys'}>Reload API keys</a>
                  </p>
                ) : keys.length ? (
                  <table id="key-list" className="key-register">
                    <thead>
                      <tr>
                        <th scope="col">Name / prefix</th>
                        <th scope="col">Scopes</th>
                        <th scope="col">Expiration</th>
                        <th scope="col">
                          <span className="sr-only">Actions</span>
                        </th>
                      </tr>
                    </thead>
                    <tbody>
                      {keys.map((key) => (
                        <tr key={key.id}>
                          <th scope="row">
                            <strong>{key.name}</strong>
                            <code>{key.prefix}…</code>
                          </th>
                          <td>
                            <span className="mobile-label">Scopes</span>
                            <code>{key.scopes.join(' ') || 'No scopes'}</code>
                          </td>
                          <td>
                            <span className="mobile-label">Expiration</span>
                            {key.expires_at ? date(key.expires_at) : 'No expiration'}
                          </td>
                          <td className="register-action">
                            <button
                              className="secondary-button"
                              type="button"
                              aria-label={
                                key.revoked_at !== null
                                  ? `${key.name} is revoked`
                                  : `Revoke ${key.name}`
                              }
                              disabled={busy || key.revoked_at !== null}
                              onClick={() => {
                                if (confirm(`Revoke ${key.name}?`))
                                  action(async () => {
                                    await api('/api/keys/' + key.id, 'DELETE');
                                    setKeys(await api('/api/keys'));
                                  });
                              }}
                            >
                              {key.revoked_at !== null ? 'Revoked' : 'Revoke'}
                            </button>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                ) : (
                  <EmptyState title="No API keys yet">
                    Create a key below to connect an app to your account.
                  </EmptyState>
                )}
              </section>
              <section className="content-section" aria-labelledby="create-key-heading">
                <div className="section-heading">
                  <h2 id="create-key-heading">Create a key</h2>
                  <p>Choose a name, the scopes you need, and an optional expiration date.</p>
                </div>
                <form id="create-key" className="key-form" onSubmit={createKey} aria-busy={busy}>
                  <label className="form-field">
                    Name
                    <input
                      name="name"
                      required
                      maxLength={128}
                      placeholder="Development laptop"
                      autoComplete="off"
                    />
                  </label>
                  <label className="form-field">
                    Scopes
                    <input
                      name="scopes"
                      placeholder="projects:read projects:write"
                      aria-describedby="scope-help"
                      autoCapitalize="none"
                      spellCheck={false}
                    />
                    <small id="scope-help">Separate scopes with spaces.</small>
                  </label>
                  <label className="form-field">
                    Expires at (optional)
                    <input name="expires" type="datetime-local" />
                  </label>
                  <div className="form-actions">
                    <button type="submit" disabled={busy}>
                      Create key
                    </button>
                  </div>
                </form>
                {secret && (
                  <section
                    id="new-secret"
                    className="secret-panel"
                    aria-labelledby="secret-heading"
                  >
                    <h3 id="secret-heading">Your new key</h3>
                    <p>Copy this key now. It will not be shown again.</p>
                    <code id="secret">{secret}</code>
                    <div className="button-group">
                      <button
                        id="copy-secret"
                        type="button"
                        disabled={busy}
                        onClick={() =>
                          action(async () => {
                            await navigator.clipboard.writeText(secret);
                            setNotice('Key copied.');
                          })
                        }
                      >
                        Copy key
                      </button>
                      <button
                        id="dismiss-secret"
                        className="secondary-button"
                        type="button"
                        onClick={() => setSecret('')}
                      >
                        Dismiss
                      </button>
                    </div>
                  </section>
                )}
              </section>
            </section>
          )}
          {authenticated && view === 'sessions' && (
            <section id="sessions" className="account-view">
              <PageHeading title="Sessions">
                Review your sign-ins and revoke access you no longer need.
              </PageHeading>
              <section
                className="content-section"
                aria-labelledby="session-list-heading"
                aria-busy={collectionState === 'loading'}
              >
                <div className="collection-heading">
                  <h2 id="session-list-heading">Your sessions</h2>
                  <button
                    id="revoke-all"
                    className="secondary-button"
                    type="button"
                    disabled={
                      busy ||
                      collectionState !== 'ready' ||
                      !sessions.some((session) => session.revoked_at === null)
                    }
                    onClick={() => {
                      if (confirm('Revoke every session, including this one?'))
                        action(async () => {
                          await api('/api/sessions', 'DELETE');
                          setMe(null);
                          setSecret('');
                        });
                    }}
                  >
                    Revoke all sessions
                  </button>
                </div>
                {collectionState === 'loading' ? (
                  <LoadingState label="Loading sessions…" />
                ) : collectionState === 'error' ? (
                  <p className="collection-error">
                    Sessions could not be loaded. <a href={prefix + '/sessions'}>Reload sessions</a>
                  </p>
                ) : sessions.length ? (
                  <ul id="session-list" className="resource-list">
                    {sessions.map((session) => (
                      <ResourceRow
                        key={session.id}
                        title={session.id === me.session_id ? 'Current session' : 'Session'}
                        action={
                          <button
                            className="secondary-button"
                            type="button"
                            aria-label={
                              session.revoked_at !== null
                                ? 'Session is revoked'
                                : session.id === me.session_id
                                  ? 'Revoke current session'
                                  : `Revoke session created ${date(session.created_at)}`
                            }
                            disabled={busy || session.revoked_at !== null}
                            onClick={() => {
                              if (confirm('Revoke this session?'))
                                action(async () => {
                                  await api('/api/sessions/' + session.id, 'DELETE');
                                  if (session.id === me.session_id) {
                                    setMe(null);
                                    setSecret('');
                                  } else setSessions(await api('/api/sessions'));
                                });
                            }}
                          >
                            {session.revoked_at !== null ? 'Revoked' : 'Revoke'}
                          </button>
                        }
                      >
                        <span>Created {date(session.created_at)}</span>
                        <span>Expires {date(session.expires_at)}</span>
                      </ResourceRow>
                    ))}
                  </ul>
                ) : (
                  <EmptyState title="No sessions yet">
                    Your sign-in sessions will appear here.
                  </EmptyState>
                )}
              </section>
            </section>
          )}
        </div>
      </main>
      <footer className="account-footer">Secured by Stargate</footer>
    </div>
  );
}
