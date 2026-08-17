export class AlchetronClient {
  constructor(baseUrl = "http://127.0.0.1:8791", token = "") {
    this.baseUrl = baseUrl.replace(/\/$/, "");
    this.token = token;
  }

  async request(path, options = {}) {
    const headers = { "content-type": "application/json", ...(options.headers || {}) };
    if (this.token) headers.authorization = `Bearer ${this.token}`;
    const response = await fetch(`${this.baseUrl}/api/v1${path}`, { ...options, headers });
    const body = await response.json();
    if (!response.ok) throw new Error(body.error || `Alchetron returned ${response.status}`);
    return body;
  }

  capabilities() { return this.request("/capabilities"); }
  catalog() { return this.request("/catalog"); }
  graph() { return this.request("/graph"); }
  publicLogs() { return this.request("/emergent/logs"); }
  hypotheses() { return this.request("/emergent/hypotheses"); }
  dashboard() { return this.request("/operator/dashboard"); }
  acceptCandidate(id, decision) {
    return this.request(`/operator/queue/${encodeURIComponent(id)}/accept`, {
      method: "POST", body: JSON.stringify(decision)
    });
  }
}
