# Use Case 2: Shared Team Vault

**Scenario:** Team project needs to share secrets (API keys, database credentials, deploy tokens). No single admin holds "the key" - access is controlled by group membership.

**Architecture:**
```
Team Members → Contextual.near (membership) → Key Manager (TEE) → FastKV
                      ↓                              ↓
              Access Control                  Same group key
              (who can access)               (what decrypts data)
```

## Why This Matters

- **No admin key:** Revoking access = remove from group, done
- **Shared access:** Anyone in group can encrypt/decrypt
- **Audit trail:** Every access logged on-chain
- **Zero trust:** Even the app developer can't access secrets

## Example: CI/CD Pipeline Secrets

```yaml
# .github/workflows/deploy.yml
- name: Load secrets from vault
  env:
    NEAR_ACCOUNT: ${{ secrets.NEAR_ACCOUNT }}
  run: |
    # CI runner calls key-manager with account credentials
    SECRETS=$(node scripts/load-vault.js myproject.near/secrets)

    # Inject into environment
    echo "DB_PASSWORD=$(echo $SECRETS | jq -r .db_password)" >> $GITHUB_ENV
    echo "API_KEY=$(echo $SECRETS | jq -r .api_key)" >> $GITHUB_ENV
```
