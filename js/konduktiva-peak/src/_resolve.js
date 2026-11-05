'use strict';
// ---------------------------------------------------------------------------
// _resolve.js -- locate host dependencies for the bundled examples and tests.
//
// konduktiva-peak does NOT hard-depend on a specific konduktiva build: the host
// project injects its own konduktiva via createPeakKit(K). konduktiva stays a
// peerDependency for that reason, and is additionally a devDependency here so
// the bundled examples and tests can resolve it in-tree.
//
// Both dependencies resolve by module id and nothing else. There is deliberately
// no fallback to an absolute path inside a sibling checkout. This file used to
// fall back to '/home/renick/claude/konduktiva-unified/src/index.js' and to that
// tree's node_modules/easymidi, which broke the moment the core directory was
// renamed to 'konduktiva'. Worse, the failure arrived as a confusing error from
// deep inside a require rather than as a clear statement that this package was
// never installed. A hardcoded path pins one machine's directory layout, so it
// is a bug waiting for the next rename.
// ---------------------------------------------------------------------------

/** Explain the fix rather than letting a bare MODULE_NOT_FOUND surface. */
function explain(name, err) {
    return new Error(
        `konduktiva-peak cannot resolve '${name}'. Run 'npm install' in `
        + `${__dirname}/.. so the bundled examples and tests can resolve it, `
        + `or require this package from a host project that provides it. `
        + `Original error: ${err && err.message}`);
}

/**
 * Resolve the host konduktiva module.
 * @returns {object} the konduktiva namespace (QuantizedMap, Player, etc.)
 */
function resolveKonduktiva() {
    try {
        return require('konduktiva');
    } catch (err) {
        throw explain('konduktiva', err);
    }
}

/**
 * Resolve the easymidi module.
 * @returns {object} the easymidi module.
 */
function resolveEasymidi() {
    try {
        return require('easymidi');
    } catch (err) {
        throw explain('easymidi', err);
    }
}

module.exports = { resolveKonduktiva, resolveEasymidi };
