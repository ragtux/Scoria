#!/usr/bin/env node
// Ports Prism's grammars to `crates/scoria-highlight`, and records Prism's own
// rendering of the samples beside this script as the crate's second corpus.
//
//     npm pack prismjs@1.29.0 && tar -xzf prismjs-1.29.0.tgz
//     node tools/prism-port/port.js package
//
// Prism builds most of its languages at load time — `extend` clones a
// grammar, `insertBefore` splices keys into the middle of one, extras like
// `js-extras` reach into another language after the fact — so the grammar a
// language *ends up* with is only known once everything has loaded. This
// script loads Prism the way Obsidian's copy is observed to be loaded, walks
// the finished grammar objects, and writes each one out as the `static` data
// the crate's engine runs, key for key and in Prism's order. Writing the
// result rather than re-deriving the chain by hand is what keeps the order
// right, and the order is load-bearing.
//
// The load order is Obsidian's as far as `tests/corpus/obsidian-1.14.5.json`
// can tell: `prism.js` (markup, css, clike, javascript) first, then every
// other component through Prism's own loader — except `regex`, which comes
// last and on its own. Obsidian's JavaScript has no `regex` inside its
// `regex-source` (the corpus shows `ab+c` untokenized), which is what loading
// `regex` after `javascript` without the loader's re-load gives. With that
// order Prism reproduces every Obsidian sample byte for byte.
//
// Prism is MIT-licensed (https://github.com/PrismJS/prism); each generated
// file names the components it was ported from.

'use strict';
const fs = require('fs');
const path = require('path');

const pkg = path.resolve(process.argv[2] || 'package');
const root = path.resolve(__dirname, '..', '..');
const crate = path.join(root, 'crates', 'scoria-highlight');
const outDir = path.join(crate, 'src', 'languages');
const PRISM_VERSION = require(path.join(pkg, 'package.json')).version;

// ---- load Prism as Obsidian does ------------------------------------------

const Prism = require(path.join(pkg, 'prism.js'));
const loadLanguages = require(path.join(pkg, 'components', 'index.js'));
const components = require(path.join(pkg, 'components.json')).languages;
const CORE = ['markup', 'css', 'clike', 'javascript'];
loadLanguages(
	Object.keys(components).filter((k) => k !== 'meta' && !CORE.includes(k) && k !== 'regex')
);
require(path.join(pkg, 'components', 'prism-regex.js'));

// ---- what to port ---------------------------------------------------------

// Languages ported by hand before this script existed. A generated grammar
// that reaches one of these refers to the hand port instead of copying it.
const HAND_PORTED = {
	clike: 'clike::CLIKE',
	css: 'web::CSS',
	diff: 'data::DIFF',
	go: 'clike::GO',
	ini: 'data::INI',
	json: 'data::JSON',
	python: 'scripting::PYTHON',
	rust: 'scripting::RUST',
	toml: 'data::TOML',
	yaml: 'data::YAML',
};

// The generated languages, in the order their objects are claimed: a grammar
// object reached from two languages is written into the first one's file.
const PORT = [
	'markup',
	// `xml` is not an alias of `markup` but a clone of it, taken before
	// `css` and `javascript` gave markup its `<style>` and `<script>`.
	'xml',
	'javascript',
	'typescript',
	'jsx',
	'tsx',
	'bash',
	'shell-session',
	'c',
	'cpp',
	'csharp',
	'java',
	'kotlin',
	'scala',
	'swift',
	'objectivec',
	'dart',
	'ruby',
	'php',
	'perl',
	'lua',
	'r',
	'julia',
	'haskell',
	'elixir',
	'erlang',
	'clojure',
	'ocaml',
	'fsharp',
	'zig',
	'sql',
	'graphql',
	'markdown',
	'latex',
	'powershell',
	'batch',
	'makefile',
	'docker',
	'nix',
	'scss',
	'less',
	'regex',
	'json5',
	'protobuf',
	'hcl',
	'nginx',
	'properties',
	'git',
	'csv',
	'vim',
	'groovy',
	'matlab',
	'arduino',
	'cmake',
	'coffeescript',
	'crystal',
	'd',
	'elm',
	'fortran',
	'glsl',
	'lisp',
	'nasm',
	'nim',
	'pascal',
	'prolog',
	'racket',
	'sass',
	'scheme',
	'solidity',
	'tcl',
	'vbnet',
	'verilog',
	'vhdl',
	'wasm',
	'apacheconf',
	'systemd',
];

const modName = (id) => id.replace(/-/g, '_');
const constName = (s) => s.replace(/[^A-Za-z0-9]+/g, '_').replace(/^_|_$/g, '').toUpperCase();

// ---- JavaScript regex to fancy-regex ---------------------------------------

const RUST_META = new Set('\\.+*?()|[]{}^$#&-~'.split(''));

function escapeLiteral(ch) {
	return RUST_META.has(ch) ? '\\' + ch : ch;
}

function translate(re) {
	// JavaScript matches an astral character as two UTF-16 surrogates; Rust
	// matches code points, and has no surrogates to name.
	const src = re.source
		.split('[\\uD800-\\uDFFF]{2}').join('[\\u{10000}-\\u{10FFFF}]')
		.split('[\\uD800-\\uDBFF][\\uDC00-\\uDFFF]').join('[\\u{10000}-\\u{10FFFF}]');
	if (/\\u[dD][89a-fA-F]/.test(src)) throw new Error('unhandled surrogate in ' + src);
	let out = '';
	let inClass = false;
	let i = 0;
	while (i < src.length) {
		const c = src[i];
		if (c === '\\') {
			const n = src[i + 1];
			if (n === 'u') {
				if (src[i + 2] === '{') {
					const j = src.indexOf('}', i);
					out += '\\x{' + src.slice(i + 3, j) + '}';
					i = j + 1;
				} else {
					out += '\\x{' + src.slice(i + 2, i + 6) + '}';
					i += 6;
				}
				continue;
			}
			if (n === 'x') {
				out += '\\x' + src.slice(i + 2, i + 4);
				i += 4;
				continue;
			}
			if (n === 'c') {
				out += '\\x{' + (src.charCodeAt(i + 2) % 32).toString(16) + '}';
				i += 3;
				continue;
			}
			if (n === '0' && !/[0-9]/.test(src[i + 2] || '')) {
				out += '\\x00';
				i += 2;
				continue;
			}
			if (/[1-9]/.test(n)) {
				let j = i + 1;
				while (/[0-9]/.test(src[j] || '')) j++;
				out += src.slice(i, j);
				i = j;
				continue;
			}
			if (n === 'b' && inClass) {
				out += '\\x08';
				i += 2;
				continue;
			}
			if (/[dDwWsSbBnrtfv]/.test(n)) {
				out += '\\' + n;
				i += 2;
				continue;
			}
			if ((n === 'p' || n === 'P') && src[i + 2] === '{') {
				const j = src.indexOf('}', i);
				out += src.slice(i, j + 1);
				i = j + 1;
				continue;
			}
			if (n === 'k' && src[i + 2] === '<') {
				const j = src.indexOf('>', i);
				out += src.slice(i, j + 1);
				i = j + 1;
				continue;
			}
			// An identity escape: `\/`, `\-`, `\<`. Rust reads `\<` as a word
			// boundary and refuses most other escapes of non-meta characters,
			// so the character goes in bare unless Rust needs it escaped.
			out += escapeLiteral(n);
			i += 2;
			continue;
		}
		if (inClass) {
			if (c === ']') {
				inClass = false;
				out += ']';
			} else if (c === '[' || c === '&' || c === '~') {
				// Rust reads a nested `[` as a class and `&&`, `~~` as set
				// operations; JavaScript reads all three as themselves.
				out += '\\' + c;
			} else if (c === '-' && src[i + 1] === '-') {
				out += '\\-';
			} else {
				out += c;
			}
			i++;
			continue;
		}
		if (c === '[') {
			if (src.startsWith('[]', i)) {
				out += '[^\\s\\S]';
				i += 2;
				continue;
			}
			if (src.startsWith('[^]', i)) {
				out += '[\\s\\S]';
				i += 3;
				continue;
			}
			inClass = true;
			out += '[';
			i++;
			if (src[i] === '^') {
				out += '^';
				i++;
			}
			continue;
		}
		if (c === '{') {
			const q = /^\{\d+(?:,\d*)?\}/.exec(src.slice(i));
			if (q) {
				out += q[0];
				i += q[0].length;
			} else {
				out += '\\{';
				i++;
			}
			continue;
		}
		if (c === '}' || c === ']') {
			out += '\\' + c;
			i++;
			continue;
		}
		out += c;
		i++;
	}
	let flags = '';
	if (re.flags.includes('i')) flags += 'i';
	if (re.flags.includes('m')) flags += 'm';
	if (re.flags.includes('s')) flags += 's';
	return (flags ? '(?' + flags + ')' : '') + out;
}

/// A Rust raw string literal for `s`.
function raw(s) {
	let hashes = '';
	while (s.includes('"' + hashes)) hashes += '#';
	return 'r' + hashes + '"' + s + '"' + hashes;
}

/// The source as the pieces a `&[&str]` holds, cut at roughly 90 characters
/// and preferably after an alternation's `|`, so a long keyword list reads
/// one line at a time the way the hand ports are written.
function pieces(s) {
	const out = [];
	while (s.length > 90) {
		let cut = s.lastIndexOf('|', 90);
		if (cut < 40) cut = 90;
		else cut += 1;
		// Never split a `\` from what it escapes.
		while (cut < s.length && s[cut - 1] === '\\') cut++;
		out.push(s.slice(0, cut));
		s = s.slice(cut);
	}
	out.push(s);
	return out;
}

// ---- walking the grammars -------------------------------------------------

const owner = new Map(); // grammar object -> { mod, name }
const files = new Map(); // mod -> { id, grammars: [{obj, name}] , names:Set }

for (const [id, path_] of Object.entries(HAND_PORTED)) {
	const [mod, name] = path_.split('::');
	owner.set(Prism.languages[id], { mod, name, hand: true });
}

function patternsOf(value) {
	return (Array.isArray(value) ? value : [value]).filter(Boolean);
}

function entriesOf(grammar) {
	const out = [];
	for (const key of Object.keys(grammar)) {
		if (key === 'rest' || !grammar[key]) continue;
		out.push([key, grammar[key]]);
	}
	return out;
}

/// A grammar that is nothing but a `rest` (`inside: { rest: x }`) is `x`.
function canonical(obj) {
	while (obj.rest && entriesOf(obj).length === 0) obj = obj.rest;
	return obj;
}

function claim(obj, mod, name) {
	obj = canonical(obj);
	if (owner.has(obj)) return;
	const file = files.get(mod);
	let unique = name;
	for (let n = 2; file.names.has(unique); n++) unique = name + '_' + n;
	file.names.add(unique);
	owner.set(obj, { mod, name: unique });
	file.grammars.push(obj);
	visit(obj, mod, unique);
}

function visit(grammar, mod, name) {
	if (grammar.rest) claim(grammar.rest, mod, name + '_REST');
	for (const [key, value] of entriesOf(grammar)) {
		const pats = patternsOf(value);
		pats.forEach((pat, ix) => {
			if (pat instanceof RegExp || !pat.inside) return;
			const suffix = pats.length > 1 ? '_' + (ix + 1) : '';
			claim(pat.inside, mod, name + '_' + constName(key) + suffix);
		});
	}
}

// Every language's own grammar is claimed first, so one language reaching
// another's top level (markup's `<script>` holds JavaScript) refers to it
// rather than taking a copy into its own file.
for (const id of PORT) {
	const grammar = Prism.languages[id];
	if (!grammar) throw new Error('Prism has no ' + id);
	if (owner.has(grammar)) throw new Error(id + ' is the same grammar as another language');
	const mod = modName(id);
	const name = constName(id);
	files.set(mod, { id, grammars: [grammar], names: new Set([name]) });
	owner.set(grammar, { mod, name });
}
for (const id of PORT) {
	visit(Prism.languages[id], modName(id), constName(id));
}

function ref(obj, fromMod) {
	const o = owner.get(canonical(obj));
	if (!o) throw new Error('unowned grammar');
	if (o.hand || o.mod !== fromMod) return '&super::' + o.mod + '::' + o.name;
	return '&' + o.name;
}

// ---- writing Rust ---------------------------------------------------------

function flagExpr(pat) {
	const f = [];
	if (pat.lookbehind) f.push('LB');
	if (pat.greedy) f.push('GR');
	return f.length ? f.join(' | ') : '0';
}

function patternRust(pat, mod, used) {
	const re = pat instanceof RegExp ? pat : pat.pattern;
	const src = '&[' + pieces(translate(re)).map(raw).join(', ') + ']';
	if (pat instanceof RegExp) {
		used.add('p');
		return 'p(' + src + ')';
	}
	const flags = flagExpr(pat);
	if (pat.lookbehind) used.add('LB');
	if (pat.greedy) used.add('GR');
	const alias = pat.alias ? [].concat(pat.alias) : [];
	const aliasRust = '&[' + alias.map((a) => JSON.stringify(a)).join(', ') + ']';
	if (pat.inside && alias.length) {
		used.add('pai');
		return 'pai(' + src + ', ' + flags + ', ' + aliasRust + ', ' + ref(pat.inside, mod) + ')';
	}
	if (pat.inside) {
		used.add('pi');
		return 'pi(' + src + ', ' + flags + ', ' + ref(pat.inside, mod) + ')';
	}
	if (alias.length) {
		used.add('pa');
		return 'pa(' + src + ', ' + flags + ', ' + aliasRust + ')';
	}
	if (flags === '0') {
		used.add('p');
		return 'p(' + src + ')';
	}
	used.add('pf');
	return 'pf(' + src + ', ' + flags + ')';
}

/// A grammar's entries with Prism's `rest` folded the way `tokenize` folds
/// it: a key the rest also has is overwritten where it stands. When nothing
/// overlaps the rest is kept as a reference, which is the crate's own
/// `Grammar::with_rest`.
function grammarRust(grammar, name, mod, isTop, used) {
	let entries = entriesOf(grammar);
	let rest = null;
	if (grammar.rest) {
		const restEntries = entriesOf(grammar.rest);
		const overlap = restEntries.some(([k]) => entries.some(([e]) => e === k));
		if (overlap || grammar.rest.rest) {
			const map = new Map(entries);
			for (const [k, v] of restEntries) map.set(k, v);
			entries = [...map.entries()];
		} else {
			rest = grammar.rest;
		}
	}
	used.add('e');
	const body = entries
		.map(([k, v]) => {
			const pats = patternsOf(v).map((p) => patternRust(p, mod, used));
			return '    e(' + JSON.stringify(k) + ', &[' + pats.join(', ') + ']),';
		})
		.join('\n');
	const vis = isTop ? 'pub' : 'pub(super)';
	const ctor = rest
		? 'Grammar::with_rest(\n    &[\n' + body + '\n    ],\n    ' + ref(rest, mod) + ',\n)'
		: 'Grammar::new(&[\n' + body + '\n])';
	return vis + ' static ' + name + ': Grammar = ' + ctor + ';\n';
}

function componentChain(id) {
	const seen = new Set();
	const order = [];
	const walk = (x) => {
		if (seen.has(x)) return;
		seen.add(x);
		const c = components[x];
		for (const r of [].concat((c && c.require) || [])) walk(r);
		order.push(x);
	};
	walk(id);
	const modifiers = Object.keys(components).filter(
		(k) => k !== 'meta' && [].concat(components[k].modify || []).some((m) => seen.has(m))
	);
	return { order, modifiers: modifiers.filter((m) => !seen.has(m)) };
}

function aliasesOf(id) {
	const g = Prism.languages[id];
	return Object.keys(Prism.languages)
		.filter((k) => k !== id && Prism.languages[k] === g && k === k.toLowerCase())
		.sort();
}

const table = [];
for (const [mod, file] of files) {
	const id = file.id;
	const used = new Set();
	const statics = file.grammars
		.map((g) => grammarRust(g, owner.get(g).name, mod, g === Prism.languages[id], used))
		.join('\n');
	// A language defined inside another's component: `xml` is in markup's.
	const comp = components[id] ? id : 'markup';
	const { order, modifiers } = componentChain(comp);
	const title = components[id] ? components[id].title : components[comp].aliasTitles[id];
	const comps = order.concat(modifiers).map((c) => '`prism-' + c + '.js`');
	const aliases = aliasesOf(id);
	const imports = ['Grammar']
		.concat(['e', 'p', 'pa', 'pai', 'pf', 'pi'].filter((x) => used.has(x)))
		.concat(['GR', 'LB'].filter((x) => used.has(x)));
	const header =
		'//! ' + title + (aliases.length ? ' (' + aliases.map((a) => '`' + a + '`').join(', ') + ')' : '') + '.\n' +
		'//!\n' +
		'//! Ported from Prism ' + PRISM_VERSION + "'s " + listJoin(comps) +
		(comps.length > 1 ? ',\n//! as the grammar stands once all of them have loaded.' : '.') +
		' Written by\n' +
		'//! `tools/prism-port/port.js`, which walks the finished grammar objects;\n' +
		'//! change that script and re-run it rather than editing this file.\n\n' +
		'use crate::grammar::{' + imports.slice(1).concat(['Grammar']).sort(sortImports).join(', ') + '};\n\n';
	fs.writeFileSync(path.join(outDir, mod + '.rs'), header + statics);
	table.push([id, aliases, mod + '::' + constName(id)]);
}

function sortImports(a, b) {
	return a < b ? -1 : a > b ? 1 : 0;
}

function listJoin(xs) {
	const lines = [];
	let line = '';
	xs.forEach((x, i) => {
		const sep = i === 0 ? '' : i === xs.length - 1 ? ' and ' : ', ';
		line += sep + x;
	});
	// Wrap at 70 columns, on the separators.
	const words = line.split(' ');
	let cur = '';
	for (const w of words) {
		if ((cur + ' ' + w).length > 68 && cur) {
			lines.push(cur);
			cur = w;
		} else cur = cur ? cur + ' ' + w : w;
	}
	lines.push(cur);
	return lines.join('\n//! ');
}

// Every alias Prism registers for a hand-ported language, too.
for (const id of Object.keys(HAND_PORTED)) {
	table.push([id, aliasesOf(id), HAND_PORTED[id]]);
}
table.sort((a, b) => (a[0] < b[0] ? -1 : 1));
const mods = [...new Set(['clike', 'data', 'scripting', 'web'].concat([...files.keys()]))].sort();
fs.writeFileSync(
	path.join(outDir, 'mod.rs'),
	`//! The ported Prism grammars, and the names a fence's info string may use.
//!
//! \`clike\`, \`data\`, \`scripting\` and \`web\` were ported by hand; every other
//! module, and this file, is written by \`tools/prism-port/port.js\` from
//! Prism ${PRISM_VERSION}'s finished grammars. Re-run the script rather than
//! editing them.

${mods.map((m) => 'pub mod ' + m + ';').join('\n')}

use crate::grammar::Grammar;

/// Every language this crate highlights, by the name Prism registers it
/// under, with the aliases Prism gives it — every other name
/// \`Prism.languages\` holds the same grammar under.
///
/// A fence whose language is not here renders plain, which is what Obsidian
/// does for a name Prism has no grammar for.
pub static LANGUAGES: &[(&str, &[&str], &Grammar)] = &[
${table.map(([id, al, p]) => `    ("${id}", &[${al.map((a) => JSON.stringify(a)).join(', ')}], &${p}),`).join('\n')}
];

/// The grammar for a language name, or \`None\` when nothing is ported for
/// it. The name is matched as Obsidian resolves it: lowercased, and the
/// aliases count.
pub fn grammar(name: &str) -> Option<&'static Grammar> {
    resolve(name).map(|(_, g)| g)
}

/// The language a name resolves to: its id as Prism registers it, and its
/// grammar. A hook is keyed on the id — \`jsx\`'s runs for \`jsx\` and \`tsx\`
/// — so an alias has to be taken back to it.
pub fn resolve(name: &str) -> Option<(&'static str, &'static Grammar)> {
    let name = name.to_ascii_lowercase();
    LANGUAGES
        .iter()
        .find(|(id, aliases, _)| *id == name || aliases.contains(&name.as_str()))
        .map(|(id, _, g)| (*id, *g))
}
`
);

// ---- the reference corpus -------------------------------------------------

function esc(s) {
	return s
		.replace(/&/g, '&amp;')
		.replace(/</g, '&lt;')
		.replace(/>/g, '&gt;')
		.replace(/ /g, '&nbsp;');
}

/// What `innerHTML` reads back from the block Obsidian renders: Prism's
/// `Token.stringify` with the two `wrap` hooks that change markup — the
/// entity's `title` and Markdown's highlighted code blocks — and the
/// serializer's escaping, which also escapes `>`.
function html(t, language) {
	if (typeof t === 'string') return esc(t);
	if (Array.isArray(t)) return t.map((x) => html(x, language)).join('');
	const classes = ['token', t.type].concat(t.alias ? [].concat(t.alias) : []);
	let content = html(t.content, language);
	let attrs = '';
	if (t.type === 'entity') {
		const title = decode(content.replace(/&amp;/, '&'));
		attrs = ' title="' + title.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;').replace(/>/g, '&gt;') + '"';
	}
	if (t.type === 'code-block') {
		const lang = classes.map((c) => /^language-(.+)/.exec(c)).find(Boolean);
		const grammar = lang && Prism.languages[lang[1]];
		if (grammar) content = html(tokenize(stringify(t.content), lang[1]), lang[1]);
	}
	return '<span class="' + classes.join(' ') + '"' + attrs + '>' + content + '</span>';
}

function decode(s) {
	return s.replace(/&(amp|lt|gt|quot|#\d+|#x[\da-f]+);/gi, (m, c) => {
		c = c.toLowerCase();
		if (c === 'amp') return '&';
		if (c === 'lt') return '<';
		if (c === 'gt') return '>';
		if (c === 'quot') return '"';
		return String.fromCodePoint(c[1] === 'x' ? parseInt(c.slice(2), 16) : Number(c.slice(1)));
	});
}

function stringify(t) {
	if (typeof t === 'string') return t;
	if (Array.isArray(t)) return t.map(stringify).join('');
	return stringify(t.content);
}

/// `Prism.highlight` without the stringify: the tokens, with the
/// `after-tokenize` hooks run.
function tokenize(code, language) {
	const env = { code, grammar: Prism.languages[language], language };
	Prism.hooks.run('before-tokenize', env);
	env.tokens = Prism.tokenize(env.code, env.grammar);
	Prism.hooks.run('after-tokenize', env);
	return env.tokens;
}

const sampleDir = path.join(__dirname, 'samples');
const corpus = {};
for (const file of fs.readdirSync(sampleDir).sort()) {
	const lang = file.replace(/\.txt$/, '');
	if (!Prism.languages[lang]) throw new Error('sample for unknown language ' + lang);
	const source = fs.readFileSync(path.join(sampleDir, file), 'utf8').replace(/\r\n/g, '\n');
	corpus[lang] = { source, html: html(tokenize(source, lang), lang) };
}
fs.writeFileSync(
	path.join(crate, 'tests', 'corpus', 'prism-' + PRISM_VERSION + '.json'),
	JSON.stringify(corpus, null, 1) + '\n'
);
console.log('ported', files.size, 'languages;', Object.keys(corpus).length, 'samples');
