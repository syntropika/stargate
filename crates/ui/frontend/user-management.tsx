import { useCallback, useEffect, useState, type FormEvent } from 'react';

type Role = 'user' | 'administrator';
type User = { id: string; email: string | null; role: Role; disabled_at: number | null };
type UserPage = { users: User[]; next_cursor: string | null };
type Api = (path: string, method?: string, body?: unknown) => Promise<unknown>;
const message = (error: unknown) =>
  error instanceof Error ? error.message : 'The request could not be completed.';

function UserRow({
  user,
  busy,
  save,
  revoke,
}: {
  user: User;
  busy: boolean;
  save: (role: Role, disabled: boolean) => void;
  revoke: () => void;
}) {
  const [role, setRole] = useState(user.role);
  const [disabled, setDisabled] = useState(user.disabled_at !== null);
  const changed = role !== user.role || disabled !== (user.disabled_at !== null);
  return (
    <tr>
      <th scope="row">
        <strong>{user.email || 'Email not provided'}</strong>
        <code>{user.id}</code>
      </th>
      <td>
        <span className="mobile-label">Role</span>
        <select
          aria-label={`Role for ${user.email || user.id}`}
          value={role}
          onChange={(event) => setRole(event.target.value as Role)}
          disabled={busy}
        >
          <option value="user">User</option>
          <option value="administrator">Administrator</option>
        </select>
      </td>
      <td>
        <label className="user-status">
          <input
            type="checkbox"
            checked={disabled}
            onChange={(event) => setDisabled(event.target.checked)}
            disabled={busy}
          />
          Disabled<span className="sr-only">: {user.email || user.id}</span>
        </label>
      </td>
      <td className="register-action">
        <div className="button-group">
          <button
            className="secondary-button"
            type="button"
            disabled={busy || !changed}
            aria-label={`Save changes for ${user.email || user.id}`}
            onClick={() => save(role, disabled)}
          >
            Save
          </button>
          <button
            className="secondary-button"
            type="button"
            disabled={busy}
            aria-label={`Revoke access for ${user.email || user.id}`}
            onClick={revoke}
          >
            Revoke access
          </button>
        </div>
      </td>
    </tr>
  );
}

export function UserManagement({
  api,
  prefix,
  currentUserId,
}: {
  api: Api;
  prefix: string;
  currentUserId: string;
}) {
  const [users, setUsers] = useState<User[]>([]);
  const [cursor, setCursor] = useState<string | null>(null);
  const [state, setState] = useState<'loading' | 'ready' | 'error'>('loading');
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');
  const reload = useCallback(async () => {
    const page = (await api('/api/users')) as UserPage;
    setUsers(page.users);
    setCursor(page.next_cursor);
    setState('ready');
  }, [api]);
  useEffect(() => {
    let active = true;
    api('/api/users')
      .then((data) => {
        if (!active) return;
        const page = data as UserPage;
        setUsers(page.users);
        setCursor(page.next_cursor);
        setState('ready');
      })
      .catch((error) => {
        if (active) {
          setNotice(message(error));
          setState('error');
        }
      });
    return () => {
      active = false;
    };
  }, [api]);
  async function action(run: () => Promise<void>) {
    setBusy(true);
    setNotice('');
    try {
      await run();
    } catch (error) {
      setNotice(message(error));
    } finally {
      setBusy(false);
    }
  }
  async function create(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    const values = new FormData(form);
    await action(async () => {
      await api('/api/users', 'POST', {
        email: values.get('email'),
        password: values.get('password'),
        role: values.get('role'),
      });
      form.reset();
      await reload();
      setNotice('User created. Share the initial password through a private channel.');
    });
  }
  return (
    <section id="users" className="account-view">
      <div className="page-heading">
        <h1>Users</h1>
        <p>Manage accounts, administrator roles and access.</p>
      </div>
      <output className="notice" aria-live="polite">
        {notice}
      </output>
      <section
        className="content-section collection-section"
        aria-labelledby="users-heading"
        aria-busy={state === 'loading' || busy}
      >
        <div className="section-heading">
          <h2 id="users-heading">Accounts</h2>
          <p>
            Keep at least one active administrator. Revoking access ends all sessions and revokes
            API keys; the user can sign in again.
          </p>
        </div>
        {state === 'loading' ? (
          <output>Loading users…</output>
        ) : state === 'error' ? (
          <p className="collection-error">
            Users could not be loaded. <a href={prefix + '/users'}>Reload users</a>
          </p>
        ) : users.length ? (
          <table className="key-register user-register">
            <thead>
              <tr>
                <th scope="col">Email / user</th>
                <th scope="col">Role</th>
                <th scope="col">Status</th>
                <th scope="col">
                  <span className="sr-only">Actions</span>
                </th>
              </tr>
            </thead>
            <tbody>
              {users.map((user) => (
                <UserRow
                  key={user.id + user.role + String(user.disabled_at)}
                  user={user}
                  busy={busy}
                  save={(role, disabled) => {
                    if (
                      disabled &&
                      !confirm(
                        `Disable ${user.email || user.id}? Their sessions and API keys will be revoked.`,
                      )
                    )
                      return;
                    void action(async () => {
                      await api('/api/users/' + user.id, 'PATCH', { role, disabled });
                      if (user.id === currentUserId) location.assign(prefix + '/profile');
                      else {
                        await reload();
                        setNotice('User updated.');
                      }
                    });
                  }}
                  revoke={() => {
                    if (confirm(`Revoke every session and API key for ${user.email || user.id}?`))
                      void action(async () => {
                        await api('/api/users/' + user.id + '/access', 'DELETE');
                        if (user.id === currentUserId) location.assign(prefix + '/profile');
                        else setNotice('Access revoked.');
                      });
                  }}
                />
              ))}
            </tbody>
          </table>
        ) : (
          <p>No users to display.</p>
        )}
        {cursor && (
          <div className="form-actions">
            <button
              className="secondary-button"
              type="button"
              disabled={busy}
              onClick={() => {
                void action(async () => {
                  const page = (await api(
                    '/api/users?after=' + encodeURIComponent(cursor),
                  )) as UserPage;
                  setUsers((previous) => [...previous, ...page.users]);
                  setCursor(page.next_cursor);
                });
              }}
            >
              Load more users
            </button>
          </div>
        )}
      </section>
      <section className="content-section" aria-labelledby="new-user-heading">
        <div className="section-heading">
          <h2 id="new-user-heading">Create a user</h2>
          <p>
            Create a local account with an initial password. The user can change it from their
            profile.
          </p>
        </div>
        <form className="key-form" onSubmit={create} aria-busy={busy}>
          <label className="form-field">
            Email
            <input
              name="email"
              type="email"
              required
              maxLength={254}
              autoComplete="off"
              autoCapitalize="none"
              spellCheck={false}
              disabled={busy}
            />
          </label>
          <label className="form-field">
            Initial password
            <input
              name="password"
              type="password"
              required
              minLength={12}
              maxLength={1024}
              autoComplete="new-password"
              aria-describedby="initial-password-help"
              disabled={busy}
            />
            <small id="initial-password-help">Use at least 12 characters.</small>
          </label>
          <label className="form-field">
            Role
            <select name="role" defaultValue="user" disabled={busy}>
              <option value="user">User</option>
              <option value="administrator">Administrator</option>
            </select>
          </label>
          <div className="form-actions">
            <button type="submit" disabled={busy || state !== 'ready'}>
              Create user
            </button>
          </div>
        </form>
      </section>
    </section>
  );
}
