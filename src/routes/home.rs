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
          body: JSON.stringify(body)
        });
        const text = await response.text();
        result.textContent = text;
      } catch (err) {
        result.textContent = 'Error enviando login';
      }
    });
  </script>
</body>
</html>
"#)
}
