use super::*;

impl RealHelpScoutProvider {
    pub(super) async fn docs_request_inner(&self, path: &str) -> Result<Value> {
        let basic = base64_of(&format!("{}:X", self.credentials.docs_api_key));
        let url = format!(
            "{}{path}",
            self.credentials.docs_api_base.trim_end_matches('/')
        );
        let res = self
            .client
            .get(url)
            .header("Authorization", format!("Basic {basic}"))
            .send()
            .await
            .map_err(|e| -> crate::error::Error {
                HsApiError {
                    status_code: 0,
                    message: format!("Network error contacting the Docs API: {e}"),
                    friendly: "The Help Scout Docs API is unreachable. The docs mirror stays empty; everything else keeps working.".into(),
                    retryable: true,
                }
                .into()
            })?;
        let status = res.status().as_u16();
        let text = res.text().await.unwrap_or_default();
        if (200..300).contains(&status) {
            if text.is_empty() {
                return Ok(Value::Null);
            }
            return serde_json::from_str(&text).map_err(|_| {
                HsApiError {
                    status_code: status,
                    message: "Invalid JSON from the Docs API".into(),
                    friendly: "The Docs API returned an unexpected response format.".into(),
                    retryable: false,
                }
                .into()
            });
        }
        Err(HsApiError {
            status_code: status,
            message: format!("Docs API GET {path} -> {status}"),
            friendly: friendly_error(status, &text, "GET"),
            retryable: false,
        }
        .into())
    }

    pub(super) async fn docs_request_send(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value> {
        if self.credentials.docs_api_key.is_empty() {
            return Err(Error::Config(
                "Docs API publishing is unavailable on this connection - no Docs API key is \
                 configured."
                    .into(),
            ));
        }
        let basic = base64_of(&format!("{}:X", self.credentials.docs_api_key));
        let url = format!(
            "{}{path}",
            self.credentials.docs_api_base.trim_end_matches('/')
        );
        let method_upper = method.to_ascii_uppercase();
        let m: reqwest::Method = method_upper
            .parse()
            .map_err(|_| Error::Other(format!("invalid HTTP method {method}").into()))?;
        let mut req = self
            .client
            .request(m, url)
            .header("Authorization", format!("Basic {basic}"));
        if let Some(payload) = body {
            req = req
                .header("Content-Type", "application/json")
                .body(serde_json::to_string(&payload).unwrap_or_default());
        }
        let res = req.send().await.map_err(|e| -> crate::error::Error {
            HsApiError {
                status_code: 0,
                message: format!("Network error contacting the Docs API: {e}"),
                friendly: "The Help Scout Docs API is unreachable. Nothing was \
                               published; the draft stays in the review queue."
                    .into(),
                retryable: true,
            }
            .into()
        })?;
        let status = res.status().as_u16();
        let text = res.text().await.unwrap_or_default();
        if (200..300).contains(&status) {
            if text.is_empty() {
                return Ok(Value::Null);
            }
            return serde_json::from_str(&text).map_err(|_| {
                HsApiError {
                    status_code: status,
                    message: "Invalid JSON from the Docs API".into(),
                    friendly: "The Docs API returned an unexpected response format. Nothing \
                               is treated as published without a valid confirmation."
                        .into(),
                    retryable: false,
                }
                .into()
            });
        }
        Err(HsApiError {
            status_code: status,
            message: format!("Docs API {method_upper} {path} -> {status}"),
            friendly: friendly_error(status, &text, &method_upper),
            retryable: false,
        }
        .into())
    }

    pub(super) fn doc_article_from_value(remote_id: String, article: &Value) -> HsDocArticle {
        HsDocArticle {
            remote_id,
            collection_id: article["collectionId"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            category_id: article["categories"]
                .as_array()
                .and_then(|a| a.first())
                .and_then(|c| c["id"].as_str().map(|s| s.to_string())),
            number: article["number"].as_i64(),
            slug: article["slug"].as_str().map(|s| s.to_string()),
            name: article["name"].as_str().unwrap_or_default().to_string(),
            status: article["status"].as_str().map(|s| s.to_string()),
            text: article["text"].as_str().map(|s| s.to_string()),
            views: article["viewCount"].as_i64(),
            created_at: article["createdDate"].as_str().map(|s| s.to_string()),
            updated_at: article["modifiedDate"].as_str().map(|s| s.to_string()),
        }
    }

    async fn access_token(&self) -> Result<String> {
        let row = {
            let conn = self.conn_lock();
            ensure_account_row(&conn);
            token_row(&conn)
        };
        let Some(row) = row else {
            return match self.client_credentials_login_if_configured().await {
                Ok(token) => Ok(token),
                Err(_) => Err(HsApiError {
                    status_code: 401,
                    message: "No Help Scout token available".into(),
                    friendly: friendly_error(401, "", "GET"),
                    retryable: false,
                }
                .into()),
            };
        };
        if row.revoked {
            return Err(HsApiError {
                status_code: 401,
                message: "The Help Scout connection was revoked".into(),
                friendly: friendly_error(401, "", "GET"),
                retryable: false,
            }
            .into());
        }
        if row.access_token.as_deref().unwrap_or("").is_empty() {
            return match self.client_credentials_login_if_configured().await {
                Ok(token) => Ok(token),
                Err(_) => Err(HsApiError {
                    status_code: 401,
                    message: "No Help Scout token available".into(),
                    friendly: friendly_error(401, "", "GET"),
                    retryable: false,
                }
                .into()),
            };
        }
        let expires_soon = match row.expires_at.as_deref() {
            Some(exp) => match chrono::DateTime::parse_from_rfc3339(exp) {
                Ok(t) => t.timestamp() < chrono::Utc::now().timestamp() + TOKEN_EXPIRY_MARGIN_SECS,
                Err(_) => true,
            },
            None => true,
        };
        if !expires_soon {
            return Ok(row.access_token.unwrap_or_default());
        }
        if let Some(refresh) = row.refresh_token.as_ref()
            && let Ok(token) = self.refresh_tokens(refresh).await
        {
            return Ok(token);
        }
        if let Ok(token) = self.client_credentials_login_if_configured().await {
            return Ok(token);
        }
        Ok(row.access_token.unwrap_or_default())
    }

    async fn client_credentials_login_if_configured(&self) -> Result<String> {
        if !self.credentials.is_configured() {
            return Err(crate::error::Error::Config(
                "No Help Scout credentials are configured".into(),
            ));
        }
        match self.client_credentials_login().await {
            Ok(token) => {
                let conn = self.conn_lock();
                let _ = crate::audit::log_event(
                    &conn,
                    "info",
                    "helpscout",
                    "Signed in to Help Scout with the stored client credentials",
                    None,
                    None,
                );
                Ok(token)
            }
            Err(e) => {
                let conn = self.conn_lock();
                let _ = crate::audit::log_event(
                    &conn,
                    "error",
                    "helpscout",
                    "Signing in with the stored credentials failed (check the Client ID/Secret)",
                    None,
                    Some(&serde_json::json!({ "detail": e.to_string() })),
                );
                Err(e)
            }
        }
    }

    pub(crate) async fn token_request(
        &self,
        form: &[(&str, &str)],
    ) -> std::result::Result<Value, HsApiError> {
        let mut form_data: Vec<(&str, &str)> = form.to_vec();
        form_data.push(("client_id", &self.credentials.client_id));
        form_data.push(("client_secret", &self.credentials.client_secret));
        let res = self
            .client
            .post(format!("{}/v2/oauth2/token", self.api_base))
            .timeout(Duration::from_secs(20))
            .form(&form_data)
            .send()
            .await
            .map_err(|e| HsApiError {
                status_code: 0,
                message: format!("OAuth token request failed: {e}"),
                friendly: "Help Scout rejected the login credentials. Check your Client ID/Secret in Settings, then try connecting again.".into(),
                retryable: false,
            })?;
        let status = res.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(HsApiError {
                status_code: status,
                message: format!("OAuth token request failed ({status})"),
                friendly: "Help Scout rejected the login credentials. Check your Client ID/Secret in Settings, then try connecting again.".into(),
                retryable: false,
            });
        }
        res.json::<Value>()
            .await
            .map_err(|e| HsApiError {
                status_code: 0,
                message: format!("invalid token response: {e}"),
                friendly: "Help Scout returned an unexpected response format. The raw response was preserved for diagnostics.".into(),
                retryable: false,
            })
    }

    pub(crate) fn save_tokens(
        &self,
        access_token: &str,
        refresh_token: Option<&str>,
        expires_in: Option<i64>,
        scope: Option<&str>,
    ) -> Result<()> {
        let expires_at =
            chrono::Utc::now() + chrono::Duration::seconds(expires_in.unwrap_or(172_800));
        let expires_iso = expires_at.to_rfc3339();
        let conn = self.conn_lock();
        ensure_account_row(&conn);
        crate::settings::set_secret(&conn, SECRET_OAUTH_ACCESS, access_token.as_bytes())?;
        if let Some(refresh) = refresh_token.filter(|r| !r.is_empty()) {
            crate::settings::set_secret(&conn, SECRET_OAUTH_REFRESH, refresh.as_bytes())?;
        }
        conn.execute(
            "INSERT INTO oauth_tokens (id, account, access_token, refresh_token, token_type, expires_at, obtained_at, scope, revoked)
             VALUES (1, 'default', '', NULL, 'bearer', ?1, datetime('now'), ?2, 0)
             ON CONFLICT(account) DO UPDATE SET access_token = '',
               refresh_token = NULL,
               expires_at = excluded.expires_at, obtained_at = datetime('now'),
               scope = excluded.scope, revoked = 0",
            rusqlite::params![expires_iso, scope],
        )?;
        Ok(())
    }

    pub(crate) async fn refresh_tokens(&self, refresh_token: &str) -> Result<String> {
        let v = self
            .token_request(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
            ])
            .await
            .map_err(Error::from)?;
        let access = v["access_token"].as_str().unwrap_or_default().to_string();
        let refresh = v["refresh_token"].as_str().map(|s| s.to_string());
        let expires = v["expires_in"].as_i64();
        let scope = v["scope"].as_str().map(|s| s.to_string());
        self.save_tokens(&access, refresh.as_deref(), expires, scope.as_deref())?;
        Ok(access)
    }

    pub async fn client_credentials_login(&self) -> Result<String> {
        let v = self
            .token_request(&[("grant_type", "client_credentials")])
            .await
            .map_err(Error::from)?;
        let access = v["access_token"].as_str().unwrap_or_default().to_string();
        let expires = v["expires_in"].as_i64();
        let scope = v["scope"].as_str().map(|s| s.to_string());
        self.save_tokens(&access, None, expires, scope.as_deref())?;
        Ok(access)
    }

    pub(crate) async fn exchange_code(&self, code: &str) -> Result<String> {
        let v = self
            .token_request(&[("grant_type", "authorization_code"), ("code", code)])
            .await
            .map_err(Error::from)?;
        let access = v["access_token"].as_str().unwrap_or_default().to_string();
        let refresh = v["refresh_token"].as_str().map(|s| s.to_string());
        let expires = v["expires_in"].as_i64();
        let scope = v["scope"].as_str().map(|s| s.to_string());
        self.save_tokens(&access, refresh.as_deref(), expires, scope.as_deref())?;
        Ok(access)
    }

    #[cfg(test)]
    pub fn oauth_status(&self) -> Value {
        let conn = self.conn_lock();
        ensure_account_row(&conn);
        let row = token_row(&conn);
        let authenticated = row.as_ref().is_some_and(|r| {
            !r.revoked && r.access_token.as_deref().is_some_and(|t| !t.is_empty())
        });
        json!({
            "configured": self.credentials.is_configured(),
            "authenticated": authenticated,
            "expiresAt": row.as_ref().and_then(|r| r.expires_at.clone()),
        })
    }

    pub async fn request(&self, path: &str, method: &str, body: Option<Value>) -> Result<Value> {
        let priority = if method != "GET" {
            PRIORITY_INTERACTIVE
        } else {
            PRIORITY_SYNC
        };
        self.request_pri(path, method, body, priority).await
    }

    pub(crate) async fn request_pri(
        &self,
        path: &str,
        method: &str,
        body: Option<Value>,
        priority: u8,
    ) -> Result<Value> {
        let is_write = method != "GET";
        let path = path.to_string();
        let method = method.to_string();
        let fut = self.request_inner(&path, &method, body, 3);
        self.queue.enqueue(priority, is_write, fut).await
    }

    pub(super) async fn request_inner(
        &self,
        path: &str,
        method: &str,
        body: Option<Value>,
        retries_left: u32,
    ) -> Result<Value> {
        let idempotent = !method.eq_ignore_ascii_case("POST");
        let token = self.access_token().await?;
        let url = format!("{}{path}", self.api_base);
        let m: reqwest::Method = method
            .parse()
            .map_err(|_| Error::Other(format!("invalid HTTP method {method}").into()))?;
        let mut req = self
            .client
            .request(m, &url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Accept", "application/json");
        if let Some(b) = &body {
            req = req.json(b);
        }

        let res = req.send().await;

        let res = match res {
            Ok(r) => r,
            Err(e) => {
                let msg = e.to_string();
                let never_delivered = e.is_connect();
                if retries_left > 0 && (never_delivered || idempotent) {
                    tokio::time::sleep(Duration::from_millis(1_500)).await;
                    return Box::pin(self.request_inner(path, method, body, retries_left - 1))
                        .await;
                }
                return Err(HsApiError {
                    status_code: 0,
                    message: format!("Network error contacting Help Scout: {msg}"),
                    friendly: if idempotent {
                        "Help Scout is unreachable. Local data remains fully browsable - you are in offline mode for remote actions.".into()
                    } else {
                        "Delivery could not be confirmed - the request was NOT re-sent automatically to avoid duplicates. Check the conversation on the platform before retrying.".into()
                    },
                    retryable: never_delivered || idempotent,
                }
                .into());
            }
        };

        let status = res.status().as_u16();
        let limit = res
            .headers()
            .get("x-ratelimit-limit-minute")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<i64>().ok());
        let remaining = res
            .headers()
            .get("x-ratelimit-remaining-minute")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<i64>().ok());
        let retry_after_hdr = res
            .headers()
            .get("x-ratelimit-retry-after")
            .or_else(|| res.headers().get("retry-after"))
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<i64>().ok());
        self.limiter
            .record_response(limit, remaining, retry_after_hdr, method != "GET");

        if status == 301 {
            let location = res
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            let new_id = location
                .split("conversations/")
                .nth(1)
                .and_then(|s| s.split(&['/', '?'][..]).next())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "another conversation".into());
            return Err(HsApiError {
                status_code: 301,
                message: format!("Conversation merged into {new_id}"),
                friendly: "This conversation was merged into another conversation in Help Scout. Open the target conversation instead.".into(),
                retryable: false,
            }
            .into());
        }

        if status == 401 && retries_left > 0 {
            let refreshed = {
                let conn = self.conn_lock();
                token_row(&conn).and_then(|r| r.refresh_token)
            };
            if let Some(refresh) = refreshed
                && self.refresh_tokens(&refresh).await.is_ok()
            {
                return Box::pin(self.request_inner(path, method, body, retries_left - 1)).await;
            }
        }

        if status == 429 {
            let retry_after = retry_after_hdr.filter(|s| *s > 0).unwrap_or(30).min(300);
            self.limiter.record_error_429(Some(retry_after));
            if retries_left > 0 {
                tokio::time::sleep(
                    Duration::from_secs(retry_after as u64) + Duration::from_millis(250),
                )
                .await;
                return Box::pin(self.request_inner(path, method, body, retries_left - 1)).await;
            }
        }

        if is_retryable(status) && retries_left > 0 && status != 429 && idempotent {
            tokio::time::sleep(Duration::from_secs(2)).await;
            return Box::pin(self.request_inner(path, method, body, retries_left - 1)).await;
        }

        if (200..300).contains(&status) {
            let resource_id = res
                .headers()
                .get("resource-id")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<i64>().ok());
            let text = res.text().await.unwrap_or_default();
            if text.is_empty() {
                return Ok(match resource_id {
                    Some(id) => json!({ "resourceId": id }),
                    None => Value::Null,
                });
            }
            return serde_json::from_str::<Value>(&text)
                .map(|mut v| {
                    if let Some(id) = resource_id
                        && let Some(obj) = v.as_object_mut()
                    {
                        obj.insert("resourceId".into(), json!(id));
                    }
                    v
                })
                .map_err(|_| {
                    HsApiError {
                        status_code: status,
                        message: "Invalid JSON from Help Scout".into(),
                        friendly: "Help Scout returned an unexpected response format. The raw response was preserved for diagnostics.".into(),
                        retryable: false,
                    }
                    .into()
                });
        }

        let body_text = res.text().await.unwrap_or_default();
        let detail: String = body_text.chars().take(4_000).collect();
        Err(HsApiError {
            status_code: status,
            message: format!("Help Scout {method} {path} -> {status}: {detail}"),
            friendly: friendly_error(status, &body_text, method),
            retryable: is_retryable(status)
                && (status == 429 || idempotent)
                && !matches!(status, 400 | 401 | 403 | 404 | 409 | 412 | 413 | 415 | 423),
        }
        .into())
    }

    pub(super) fn map_user(v: &Value) -> HsUser {
        HsUser {
            remote_id: v["id"].as_i64().unwrap_or(0),
            first_name: v["firstName"].as_str().map(|s| s.to_string()),
            last_name: v["lastName"].as_str().map(|s| s.to_string()),
            email: v["email"].as_str().map(|s| s.to_string()),
            role: v["role"].as_str().map(|s| s.to_string()),
            user_type: v["type"].as_str().unwrap_or("user").to_string(),
            timezone: v["timezone"].as_str().map(|s| s.to_string()),
            photo_url: v["photoUrl"].as_str().map(|s| s.to_string()),
            initials: v["initials"].as_str().map(|s| s.to_string()),
            mention: v["mention"].as_str().map(|s| s.to_string()),
            job_title: v["jobTitle"].as_str().map(|s| s.to_string()),
            phone: v["phone"].as_str().map(|s| s.to_string()),
            alternate_emails: Vec::new(),
            created_at: v["createdAt"].as_str().map(|s| s.to_string()),
            updated_at: v["updatedAt"].as_str().map(|s| s.to_string()),
        }
    }

    pub(super) fn map_mailbox(v: &Value) -> HsMailbox {
        HsMailbox {
            remote_id: v["id"].as_i64().unwrap_or(0),
            name: v["name"].as_str().unwrap_or_default().to_string(),
            slug: v["slug"].as_str().map(|s| s.to_string()),
            email: v["email"].as_str().map(|s| s.to_string()),
            created_at: v["createdAt"].as_str().map(|s| s.to_string()),
            updated_at: v["updatedAt"].as_str().map(|s| s.to_string()),
        }
    }

    pub(super) fn map_folder(v: &Value, mailbox_id: i64) -> HsFolder {
        HsFolder {
            remote_id: v["id"].as_i64().unwrap_or(0),
            mailbox_id,
            name: v["name"].as_str().unwrap_or_default().to_string(),
            kind: v["type"].as_str().unwrap_or_default().to_string(),
            user_id: v["userId"].as_i64(),
            total_count: v["totalCount"].as_i64().unwrap_or(0),
            active_count: v["activeCount"].as_i64().unwrap_or(0),
        }
    }

    pub(super) fn map_field(v: &Value, mailbox_id: i64) -> HsField {
        HsField {
            remote_id: v["id"].as_i64().unwrap_or(0),
            mailbox_id,
            name: v["name"].as_str().unwrap_or_default().to_string(),
            kind: v["type"].as_str().unwrap_or_default().to_string(),
            system_type: v["systemType"].as_str().map(|s| s.to_string()),
            required: v["required"].as_bool().unwrap_or(false),
            sort_order: v["order"].as_i64().unwrap_or(0),
            options: v["_embedded"]["fields"]
                .as_array()
                .map(|opts| {
                    opts.iter()
                        .map(|o| HsFieldOption {
                            id: o["id"].as_i64().unwrap_or(0),
                            order: o["order"].as_i64().unwrap_or(0),
                            label: o["label"].as_str().unwrap_or_default().to_string(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    pub(super) fn map_tag(v: &Value) -> HsTag {
        HsTag {
            remote_id: v["id"].as_i64().unwrap_or(0),
            name: v["tag"].as_str().unwrap_or_default().to_string(),
            slug: v["slug"].as_str().map(|s| s.to_string()),
            color: v["color"].as_str().map(|s| s.to_string()),
            ticket_count: v["ticketCount"].as_i64(),
            created_at: v["createdAt"].as_str().map(|s| s.to_string()),
            updated_at: v["updatedAt"].as_str().map(|s| s.to_string()),
        }
    }
}
