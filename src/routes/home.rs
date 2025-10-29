// routes/home.rs
// GET / -> simple HTML page with a minimal login form that posts JSON to /login.

use axum::response::Html;

pub async fn home() -> Html<&'static str> {
    Html(r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>TOTP Login</title>
</head>
<body>
  <main>
    <section>
      <strong>Estado:</strong>
      <span id="status">No autenticado</span>
      <button id="logout-button" type="button" hidden>Salir</button>
    </section>
    <form id="login-form">
      <label>
        Email
        <input id="email" name="email" type="email" required>
      </label>
      <label>
        Código
        <input id="code" name="code" inputmode="numeric" pattern="\d*" required>
      </label>
      <button type="submit">Entrar</button>
    </form>
    <pre id="result"></pre>
  </main>
  <script>
    const form = document.getElementById('login-form');
    const result = document.getElementById('result');
    const statusEl = document.getElementById('status');
    const logoutBtn = document.getElementById('logout-button');
    const STORAGE_KEY = 'currentEmail';

    function setStatus(text) {
      statusEl.textContent = text;
    }

    function toggleLogout(visible) {
      logoutBtn.hidden = !visible;
    }

    async function checkSession(email) {
      if (!email) {
        setStatus('No autenticado');
        toggleLogout(false);
        return;
      }
      try {
        const response = await fetch(`/setup?email=${encodeURIComponent(email)}`, {
          method: 'GET',
          credentials: 'same-origin'
        });
        if (response.ok) {
          const data = await response.json();
          setStatus(`Autenticado como ${data.email} (${data.company})`);
          toggleLogout(true);
        } else if (response.status === 401 || response.status === 403) {
          localStorage.removeItem(STORAGE_KEY);
          setStatus('No autenticado');
          toggleLogout(false);
        } else {
          setStatus('Sesión desconocida');
          toggleLogout(false);
        }
      } catch (err) {
        setStatus('Error consultando sesión');
        toggleLogout(false);
      }
    }

    form.addEventListener('submit', async (event) => {
      event.preventDefault();
      const body = {
        email: form.email.value.trim(),
        code: form.code.value.trim()
      };

      try {
        const response = await fetch('/login', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          credentials: 'same-origin',
          body: JSON.stringify(body)
        });
        const text = await response.text();
        try {
          const data = JSON.parse(text);
          if (response.ok && data.ok) {
            localStorage.setItem(STORAGE_KEY, body.email);
            result.textContent = 'Login correcto';
            await checkSession(body.email);
            return;
          }
          if (data && data.error) {
            result.textContent = data.error;
          } else {
            result.textContent = text;
          }
        } catch (_) {
          result.textContent = text;
        }
      } catch (err) {
        result.textContent = 'Error enviando login';
      }
    });

    logoutBtn.addEventListener('click', async () => {
      try {
        const response = await fetch('/logout', {
          method: 'POST',
          credentials: 'same-origin'
        });
        localStorage.removeItem(STORAGE_KEY);
        toggleLogout(false);
        if (response.ok) {
          result.textContent = 'Sesión cerrada';
          setStatus('No autenticado');
        } else {
          const text = await response.text();
          result.textContent = text || 'Error al cerrar sesión';
          setStatus('No autenticado');
        }
      } catch (err) {
        result.textContent = 'Error al cerrar sesión';
      }
    });

    const savedEmail = localStorage.getItem(STORAGE_KEY);
    if (savedEmail) {
      checkSession(savedEmail);
    } else {
      setStatus('No autenticado');
      toggleLogout(false);
    }
  </script>
</body>
</html>
"#)
}
