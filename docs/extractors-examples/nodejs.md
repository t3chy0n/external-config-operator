## JavaScript (Node.js): Vault-Backed Extractor

### Dependencies:
- Vault Node.js SDK.
- Express

### Implementation:
```javascript
const express = require('express');
const { Vault } = require('node-vault');

const app = express();
const port = 8080;

// Configure Vault Client
const vault = Vault({
    apiVersion: 'v1',
    endpoint: process.env.VAULT_ADDR || 'http://127.0.0.1:8200',
    token: process.env.VAULT_TOKEN || 's.YourTokenHere',
});

app.get('/config/:profile/:label', async (req, res) => {
    const { profile, label } = req.params;
    const path = `${profile}/${label}`;

    try {
        const secret = await vault.read(path);
        res.json(secret.data);
    } catch (err) {
        res.status(500).send({ error: 'Failed to fetch configuration' });
    }
});

app.listen(port, () => {
    console.log(`Vault Extractor running on http://localhost:${port}`);
});
```
