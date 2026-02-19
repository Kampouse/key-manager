/**
 * API Server - Blind to User Secrets
 *
 * This server stores encrypted user profiles but CANNOT decrypt them.
 * The encryption keys exist only inside the TEE.
 */

import express from 'express';

const app = express();
app.use(express.json());

// In production, use a real database
const profiles = new Map();

/**
 * GET /api/profile/:accountId
 *
 * Returns public + encrypted private data.
 * Server CANNOT decrypt the private fields.
 */
app.get('/api/profile/:accountId', (req, res) => {
  const { accountId } = req.params;
  const profile = profiles.get(accountId);

  if (!profile) {
    return res.status(404).json({ error: 'Profile not found' });
  }

  // Server returns encrypted blobs blindly
  // Has NO ability to decrypt them
  res.json({
    account_id: accountId,
    public: profile.public,
    private_encrypted: profile.private_encrypted,
    _note: "Private fields are encrypted. Only the owner can decrypt them."
  });
});

/**
 * POST /api/profile
 *
 * Stores profile. Server sees encrypted blobs for private fields.
 */
app.post('/api/profile', (req, res) => {
  const { account_id, public: publicFields, private_encrypted } = req.body;

  // Server stores everything but can only read public fields
  profiles.set(account_id, {
    public: publicFields,
    private_encrypted, // Just opaque strings to us
    updated_at: new Date().toISOString()
  });

  console.log(`[AUDIT] Profile updated for ${account_id}`);
  console.log(`  Public: ${JSON.stringify(publicFields)}`);
  console.log(`  Private: [ENCRYPTED - ${Object.keys(private_encrypted).length} fields]`);

  res.json({ success: true, account_id });
});

/**
 * DELETE /api/profile/:accountId
 *
 * GDPR "Right to be Forgotten" - server can delete but never read
 */
app.delete('/api/profile/:accountId', (req, res) => {
  const { accountId } = req.params;

  if (profiles.has(accountId)) {
    profiles.delete(accountId);
    console.log(`[AUDIT] Profile deleted for ${accountId}`);
    res.json({ success: true });
  } else {
    res.status(404).json({ error: 'Profile not found' });
  }
});

/**
 * GET /api/profile/:accountId/attestation
 *
 * Provides proof that keys are TEE-derived
 * (Useful for compliance audits)
 */
app.get('/api/profile/:accountId/attestation', (req, res) => {
  const { accountId } = req.params;

  res.json({
    encryption_scheme: "AES-256-GCM",
    key_management: "TEE (OutLayer)",
    key_derivation: "CKD - Confidential Key Derivation",
    attestation: "Keys derived inside TEE, never exported",
    compliance: {
      GDPR: "Controller cannot access data",
      HIPAA: "PHI encrypted at rest with TEE-managed keys",
      proof_url: "https://outlayer.fastnear.com/executions/[execution_id]"
    }
  });
});

// Start server
const PORT = process.env.PORT || 3000;
app.listen(PORT, () => {
  console.log(`Profile API running on port ${PORT}`);
  console.log(`⚠️  This server CANNOT decrypt user private data`);
  console.log(`🔐 Keys exist only inside OutLayer TEE`);
});

export default app;
