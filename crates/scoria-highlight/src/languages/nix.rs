//! Nix.
//!
//! Ported from Prism 1.29.0's `prism-nix.js`. Written by
//! `tools/prism-port/port.js`, which walks the finished grammar objects;
//! change that script and re-run it rather than editing this file.

use crate::grammar::{e, p, pa, pf, pi, Grammar, GR, LB};

pub static NIX: Grammar = Grammar::new(&[
    e("comment", &[pf(&[r"/\*[\s\S]*?\*/|#.*"], GR)]),
    e(
        "string",
        &[pi(
            &[r#""(?:[^"\\]|\\[\s\S])*"|''(?:(?!'')[\s\S]|''(?:'|\\|\$\{))*''"#],
            GR,
            &NIX_STRING,
        )],
    ),
    e(
        "url",
        &[
            p(&[r"\b(?:[a-z]{3,7}://)[\w\-+%\~/.:#=?\&]+"]),
            pf(
                &[
                    r"([^/])(?:[\w\-+%\~.:#=?\&]*(?!//)[\w\-+%\~/.:#=?\&])?(?!//)/[\w\-+%\~/.:#=?\&]*",
                ],
                LB,
            ),
        ],
    ),
    e("antiquotation", &[pa(&[r"\$(?=\{)"], 0, &["important"])]),
    e("number", &[p(&[r"\b\d+\b"])]),
    e(
        "keyword",
        &[p(&[
            r"\b(?:assert|builtins|else|if|in|inherit|let|null|or|then|with)\b",
        ])],
    ),
    e(
        "function",
        &[p(&[
            r"\b(?:abort|add|all|any|attrNames|attrValues|baseNameOf|compareVersions|concatLists|",
            r"currentSystem|deepSeq|derivation|dirOf|div|elem(?:At)?|fetch(?:Tarball|url)|",
            r"filter(?:Source)?|fromJSON|genList|getAttr|getEnv|hasAttr|hashString|head|import|",
            r"intersectAttrs|is(?:Attrs|Bool|Function|Int|List|Null|String)|length|lessThan|listToAttrs|",
            r"map|mul|parseDrvName|pathExists|read(?:Dir|File)|removeAttrs|replaceStrings|seq|sort|",
            r"stringLength|sub(?:string)?|tail|throw|to(?:File|JSON|Path|String|XML)|trace|typeOf)\b|",
            r"\bfoldl'\B",
        ])],
    ),
    e("boolean", &[p(&[r"\b(?:false|true)\b"])]),
    e("operator", &[p(&[r"[=!<>]=?|\+\+?|\|\||&&|//|->?|[?@]"])]),
    e("punctuation", &[p(&[r"[{}()\[\].,:;]"])]),
]);

pub(super) static NIX_STRING: Grammar = Grammar::new(&[e(
    "interpolation",
    &[pi(
        &[r"(^|(?:^|(?!'').)[^\\])\$\{(?:[^{}]|\{[^}]*\})*\}"],
        LB,
        &NIX,
    )],
)]);
