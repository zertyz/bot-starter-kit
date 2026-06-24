# Denied Constructs
1. Do not allow `anyhow::Context` and related `.context` & `.with_context` high-order-functions.
   Reason: they drop the error context, promoting a partial error hiding anti-pattern.
   Alternative: use `.map_err(|err| anyhow!("At operation X: {err}"))`, which correctly preserves the root cause `err`