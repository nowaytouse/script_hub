"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.default = Ansi;
const anser_1 = __importDefault(require("anser"));
const escape_carriage_1 = require("escape-carriage");
const linkify_it_1 = __importDefault(require("linkify-it"));
const React = __importStar(require("react"));
/**
 * Converts ANSI strings into JSON output.
 * @name ansiToJSON
 * @function
 * @param {String} input The input string.
 * @param {boolean} use_classes If `true`, HTML classes will be appended
 *                              to the HTML output.
 * @return {Array} The parsed input.
 */
function ansiToJSON(input, use_classes = false) {
    input = (0, escape_carriage_1.escapeCarriageReturn)(fixBackspace(input));
    return anser_1.default.ansiToJson(input, {
        json: true,
        remove_empty: true,
        use_classes,
    });
}
/**
 * Create a class string.
 * @name createClass
 * @function
 * @param {AnserJsonEntry} bundle
 * @return {String} class name(s)
 */
function createClass(bundle) {
    let classNames = "";
    if (bundle.bg) {
        classNames += `${bundle.bg}-bg `;
    }
    if (bundle.fg) {
        classNames += `${bundle.fg}-fg `;
    }
    if (bundle.decoration) {
        classNames += `ansi-${bundle.decoration} `;
    }
    if (classNames === "") {
        return null;
    }
    classNames = classNames.substring(0, classNames.length - 1);
    return classNames;
}
/**
 * Create the style attribute.
 * @name createStyle
 * @function
 * @param {AnserJsonEntry} bundle
 * @return {Object} returns the style object
 */
function createStyle(bundle) {
    const style = {};
    if (bundle.bg) {
        style.backgroundColor = `rgb(${bundle.bg})`;
    }
    if (bundle.fg) {
        style.color = `rgb(${bundle.fg})`;
    }
    switch (bundle.decoration) {
        case "bold":
            style.fontWeight = "bold";
            break;
        case "dim":
            style.opacity = "0.5";
            break;
        case "italic":
            style.fontStyle = "italic";
            break;
        case "hidden":
            style.visibility = "hidden";
            break;
        case "strikethrough":
            style.textDecoration = "line-through";
            break;
        case "underline":
            style.textDecoration = "underline";
            break;
        case "blink":
            style.textDecoration = "blink";
            break;
        default:
            break;
    }
    return style;
}
/**
 * Converts an Anser bundle into a React Node.
 * @param linkify whether links should be converting into clickable anchor tags.
 * @param useClasses should render the span with a class instead of style.
 * @param bundle Anser output.
 * @param key
 */
function convertBundleIntoReact(linkify, useClasses, bundle, key) {
    const style = useClasses ? null : createStyle(bundle);
    const className = useClasses ? createClass(bundle) : null;
    if (!linkify) {
        return React.createElement("span", { style, key, className }, bundle.content);
    }
    if (linkify === "fuzzy") {
        return linkWithLinkify(bundle, key, style, className);
    }
    return linkWithClassicMode(bundle, key, style, className);
}
function linkWithClassicMode(bundle, key, style, className) {
    const content = [];
    const linkRegex = /(\s|^)(https?:\/\/(?:www\.|(?!www))[^\s.]+\.[^\s]{2,}|www\.[^\s]+\.[^\s]{2,})/g;
    let index = 0;
    let match;
    while ((match = linkRegex.exec(bundle.content)) !== null) {
        const [, pre, url] = match;
        const startIndex = match.index + pre.length;
        if (startIndex > index) {
            content.push(bundle.content.substring(index, startIndex));
        }
        // Make sure the href we generate from the link is fully qualified. We assume http
        // if it starts with a www because many sites don't support https
        const href = url.startsWith("www.") ? `http://${url}` : url;
        content.push(React.createElement("a", {
            key: index,
            href,
            target: "_blank",
        }, `${url}`));
        index = linkRegex.lastIndex;
    }
    if (index < bundle.content.length) {
        content.push(bundle.content.substring(index));
    }
    return React.createElement("span", { style, key, className }, content);
}
function linkWithLinkify(bundle, key, style, className) {
    var _a;
    const linker = (0, linkify_it_1.default)({ fuzzyEmail: false }).tlds(["io"], true);
    if (!linker.pretest(bundle.content)) {
        return React.createElement("span", { style, key, className }, bundle.content);
    }
    const matches = linker.match(bundle.content);
    if (!matches) {
        return React.createElement("span", { style, key, className }, bundle.content);
    }
    const content = [
        bundle.content.substring(0, (_a = matches[0]) === null || _a === void 0 ? void 0 : _a.index),
    ];
    matches.forEach((match, i) => {
        var _a;
        content.push(React.createElement("a", {
            href: match.url,
            target: "_blank",
            key: i,
        }, bundle.content.substring(match.index, match.lastIndex)));
        if (matches[i + 1]) {
            content.push(bundle.content.substring(matches[i].lastIndex, (_a = matches[i + 1]) === null || _a === void 0 ? void 0 : _a.index));
        }
    });
    if (matches[matches.length - 1].lastIndex !== bundle.content.length) {
        content.push(bundle.content.substring(matches[matches.length - 1].lastIndex, bundle.content.length));
    }
    return React.createElement("span", { style, key, className }, content);
}
function Ansi(props) {
    const { className, useClasses, children, linkify } = props;
    return React.createElement("code", { className }, ansiToJSON(children !== null && children !== void 0 ? children : "", useClasses !== null && useClasses !== void 0 ? useClasses : false).map(convertBundleIntoReact.bind(null, linkify !== null && linkify !== void 0 ? linkify : false, useClasses !== null && useClasses !== void 0 ? useClasses : false)));
}
// This is copied from the Jupyter Classic source code
// notebook/static/base/js/utils.js to handle \b in a way
// that is **compatible with Jupyter classic**.   One can
// argue that this behavior is questionable:
//   https://stackoverflow.com/questions/55440152/multiple-b-doesnt-work-as-expected-in-jupyter#
function fixBackspace(txt) {
    let tmp = txt;
    do {
        txt = tmp;
        // Cancel out anything-but-newline followed by backspace
        tmp = txt.replace(/[^\n]\x08/gm, "");
    } while (tmp.length < txt.length);
    return txt;
}
