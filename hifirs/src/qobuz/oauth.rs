use crate::sql::db;
use hifirs_qobuz_api::client::api::Client as QobuzClient;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

type Result<T, E = hifirs_qobuz_api::Error> = std::result::Result<T, E>;

/// Perform an interactive OAuth login: open the browser, capture the
/// authorization code from the redirect, exchange it for a token and persist it.
pub async fn login(client: &mut QobuzClient, advertise: Option<&str>) -> Result<()> {
    if client.get_app_id().is_none() || client.get_private_key().is_none() {
        client.refresh().await?;

        if let Some(id) = client.get_app_id() {
            db::set_app_id(id).await;
        }

        if let Some(key) = client.get_private_key() {
            db::set_private_key(key).await;
        }
    }

    let app_id = client.get_app_id().cloned().ok_or(hifirs_qobuz_api::Error::AppID)?;

    let code = capture_authorization_code(&app_id, advertise).await?;

    client.login_with_oauth_code(&code).await?;

    if let Some(token) = client.get_token() {
        db::set_user_token(token).await;
    }

    Ok(())
}

async fn capture_authorization_code(
    app_id: &str,
    advertise: Option<&str>,
) -> Result<String> {
    let bind = if advertise.is_some() { "0.0.0.0" } else { "127.0.0.1" };
    let no_browser = advertise.is_some();

    let bind_addr = format!("{bind}:0");
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .map_err(|error| hifirs_qobuz_api::Error::Api {
            message: error.to_string(),
        })?;

    let addr = listener.local_addr().map_err(|error| hifirs_qobuz_api::Error::Api {
        message: error.to_string(),
    })?;

    let host = advertise.unwrap_or(bind);
    let redirect_url = format!("http://{host}:{port}", port = addr.port());
    let signin_url = format!(
        "https://www.qobuz.com/signin/oauth?ext_app_id={app_id}&redirect_url={redirect_url}"
    );

    info!("sign in to Qobuz by opening: {signin_url}");
    println!("Sign in to Qobuz by opening: {signin_url}");

    if !no_browser {
        if let Err(error) = webbrowser::open(&signin_url) {
            warn!("failed to open browser automatically: {error}");
        }
    } else {
        info!("auto-open disabled; open this URL manually: {signin_url}");
    }

    let (mut stream, _) = listener.accept().await.map_err(|error| {
        hifirs_qobuz_api::Error::Api {
            message: error.to_string(),
        }
    })?;

    let mut buffer = [0_u8; 8192];
    let n = stream.read(&mut buffer).await.map_err(|error| {
        hifirs_qobuz_api::Error::Api {
            message: error.to_string(),
        }
    })?;

    let request = String::from_utf8_lossy(&buffer[..n]).to_string();
    let code = extract_code(&request).ok_or(hifirs_qobuz_api::Error::OAuth)?;

    let body = format!("Login successful, you can close this tab. ({code})");
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );

    let _ = stream.write_all(response.as_bytes()).await;

    Ok(code)
}

fn extract_code(request: &str) -> Option<String> {
    let first_line = request.lines().next()?;
    let path = first_line.split_whitespace().nth(1)?;
    let query = path.split('?').nth(1)?;

    for pair in query.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            if key == "code" || key == "code_autorisation" {
                return Some(value.to_string());
            }
        }
    }

    None
}
