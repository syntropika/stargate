import { useState, type FormEvent } from 'react';

export type LocalStatus = {
  enabled: boolean;
  setup_available: boolean;
  csrf_token: string;
};

function failure(status: number, setup: boolean) {
  if (status === 401) return 'Email or password is incorrect.';
  if (status === 409 && setup)
    return 'Setup has already been completed. Reload this page to sign in.';
  if (status === 429) return 'Too many attempts. Wait a few minutes and try again.';
  if (status === 403) return 'Your form expired. Reload this page and try again.';
  if (status === 400) return 'Check your email and use a password with at least 12 characters.';
  return 'The request could not be completed. Please try again.';
}

export function LocalAccess({ prefix, status }: { prefix: string; status: LocalStatus }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const setup = status.setup_available;
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    const values = new FormData(form);
    if (setup && values.get('password') !== values.get('confirmation')) {
      setError('The passwords do not match.');
      return;
    }
    setBusy(true);
    setError('');
    try {
      const response = await fetch(prefix + '/api/local/' + (setup ? 'setup' : 'login'), {
        method: 'POST',
        credentials: 'same-origin',
        headers: { 'content-type': 'application/json', 'x-stargate-csrf': status.csrf_token },
        body: JSON.stringify({ email: values.get('email'), password: values.get('password') }),
      });
      if (!response.ok) throw new Error(failure(response.status, setup));
      form.reset();
      location.assign(prefix + '/profile');
    } catch (error) {
      setError(error instanceof Error ? error.message : 'Sign-in could not be completed.');
      setBusy(false);
    }
  }
  return (
    <form className="local-form" onSubmit={submit} aria-busy={busy}>
      <label className="form-field">
        Email
        <input
          name="email"
          type="email"
          required
          maxLength={254}
          autoComplete="username"
          autoCapitalize="none"
          spellCheck={false}
          disabled={busy}
        />
      </label>
      <label className="form-field">
        Password
        <input
          name="password"
          type="password"
          required
          minLength={setup ? 12 : undefined}
          maxLength={1024}
          autoComplete={setup ? 'new-password' : 'current-password'}
          aria-describedby={setup ? 'password-help' : undefined}
          disabled={busy}
        />
        {setup && <small id="password-help">Use at least 12 characters.</small>}
      </label>
      {setup && (
        <label className="form-field">
          Confirm password
          <input
            name="confirmation"
            type="password"
            required
            minLength={12}
            maxLength={1024}
            autoComplete="new-password"
            disabled={busy}
          />
        </label>
      )}
      {error && (
        <p className="collection-error" role="alert">
          {error}
        </p>
      )}
      <div className="form-actions">
        <button type="submit" disabled={busy}>
          {busy
            ? setup
              ? 'Creating account…'
              : 'Signing in…'
            : setup
              ? 'Create administrator account'
              : 'Sign in'}
        </button>
      </div>
    </form>
  );
}

export function PasswordChange({ prefix, csrf }: { prefix: string; csrf: string }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    const values = new FormData(form);
    if (values.get('new_password') !== values.get('confirmation')) {
      setError('The passwords do not match.');
      return;
    }
    setBusy(true);
    setError('');
    try {
      const response = await fetch(prefix + '/api/local/password', {
        method: 'POST',
        credentials: 'same-origin',
        headers: { 'content-type': 'application/json', 'x-stargate-csrf': csrf },
        body: JSON.stringify({
          current_password: values.get('current_password'),
          new_password: values.get('new_password'),
        }),
      });
      if (!response.ok)
        throw new Error(
          response.status === 401
            ? 'Your current password is incorrect.'
            : failure(response.status, false),
        );
      form.reset();
      location.assign(prefix + '/profile');
    } catch (error) {
      setError(error instanceof Error ? error.message : 'Password could not be changed.');
      setBusy(false);
    }
  }
  return (
    <section className="content-section" aria-labelledby="password-heading">
      <div className="section-heading">
        <h2 id="password-heading">Change password</h2>
        <p>Changing your password signs out your other sessions.</p>
      </div>
      <form className="local-form" onSubmit={submit} aria-busy={busy}>
        <label className="form-field">
          Current password
          <input
            name="current_password"
            type="password"
            required
            maxLength={1024}
            autoComplete="current-password"
            disabled={busy}
          />
        </label>
        <label className="form-field">
          New password
          <input
            name="new_password"
            type="password"
            required
            minLength={12}
            maxLength={1024}
            autoComplete="new-password"
            aria-describedby="new-password-help"
            disabled={busy}
          />
          <small id="new-password-help">Use at least 12 characters.</small>
        </label>
        <label className="form-field">
          Confirm new password
          <input
            name="confirmation"
            type="password"
            required
            minLength={12}
            maxLength={1024}
            autoComplete="new-password"
            disabled={busy}
          />
        </label>
        {error && (
          <p className="collection-error" role="alert">
            {error}
          </p>
        )}
        <div className="form-actions">
          <button type="submit" disabled={busy}>
            {busy ? 'Changing password…' : 'Change password'}
          </button>
        </div>
      </form>
    </section>
  );
}
