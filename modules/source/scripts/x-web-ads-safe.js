// SPDX-License-Identifier: GPL-3.0-only
// Endpoint and response schema reference: fmz200/wool_scripts (GPL-3.0), Scripts/X/X_ads.js.
// https://github.com/fmz200/wool_scripts/blob/main/Scripts/X/X_ads.js

(function () {
    function object(value) {
        return value !== null && typeof value === "object" && !Array.isArray(value);
    }

    function promoted(entry, content) {
        return (typeof entry.entryId === "string" && /^promoted(?:-|$)/.test(entry.entryId)) ||
            object(content && content.promotedMetadata);
    }

    function rewrite() {
        const operation = /^https:\/\/x\.com\/i\/api\/graphql\/[A-Za-z0-9_-]+\/(HomeTimeline|HomeLatestTimeline|TweetDetail)(?:\?|$)/.exec($request.url);
        const status = $response.status ?? $response.statusCode;
        if (!operation || !Number.isInteger(status) || status < 200 || status >= 300 ||
            typeof $response.body !== "string" || $response.body.length > 1048576) return {};

        const data = JSON.parse($response.body);
        if (!object(data) || data.errors?.length) return {};
        const detail = operation[1] === "TweetDetail";
        const instructions = detail
            ? data?.data?.threaded_conversation_with_injections_v2?.instructions
            : data?.data?.home?.home_timeline_urt?.instructions;
        if (!Array.isArray(instructions)) return {};

        let changed = false;
        for (const instruction of instructions) {
            if (!object(instruction)) return {};
            const entries = instruction.entries;
            if (entries === undefined) continue;
            if (!Array.isArray(entries)) return {};

            const keptEntries = [];
            for (const entry of entries) {
                if (!object(entry)) return {};
                if (promoted(entry, entry.content?.itemContent)) continue;
                if (detail && entry.content?.items !== undefined) {
                    const items = entry.content.items;
                    if (!Array.isArray(items) || !items.every(object)) return {};
                    const keptItems = items.filter(item => !promoted(item, item.item?.itemContent ?? item.itemContent));
                    if (keptItems.length !== items.length) {
                        if (!keptItems.length) continue;
                        entry.content.items = keptItems;
                        changed = true;
                    }
                }
                keptEntries.push(entry);
            }
            if (keptEntries.length !== entries.length) {
                instruction.entries = keptEntries;
                changed = true;
            }
        }
        return changed ? { body: JSON.stringify(data) } : {};
    }

    let result = {};
    try {
        result = rewrite();
    } catch {
        // Leave malformed responses, including HTML challenges, unchanged.
    }
    $done(result);
})();
