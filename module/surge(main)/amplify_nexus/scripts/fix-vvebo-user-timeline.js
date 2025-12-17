// VVebo用户时间线修复脚本 - 修复版
// 原作者: suiyuran
// 修复: passing card error问题

let url = $request.url;

let hasUid = (url) => url.includes("uid");
let getUid = (url) => (hasUid(url) ? url.match(/uid=(\d+)/)?.[1] : undefined);

if (url.includes("remind/unread_count")) {
  // 保存uid用于后续请求
  let uid = getUid(url);
  if (uid) {
    $persistentStore.write(uid, "uid");
  }
  $done({});
} else if (url.includes("statuses/user_timeline")) {
  // 重定向到profile/statuses/tab接口
  let uid = getUid(url) || $persistentStore.read("uid");
  if (!uid) {
    console.log("[VVebo修复] 错误: 无法获取uid");
    $done({});
    return;
  }
  url = url.replace("statuses/user_timeline", "profile/statuses/tab").replace("max_id", "since_id");
  url = url + `&containerid=230413${uid}_-_WEIBO_SECOND_PROFILE_WEIBO`;
  $done({ url });
} else if (url.includes("profile/statuses/tab")) {
  // 处理响应数据
  try {
    let data = JSON.parse($response.body);
    
    // 安全检查: 确保cards存在且为数组
    if (!data || !data.cards || !Array.isArray(data.cards)) {
      console.log("[VVebo修复] 警告: cards数据无效或为空");
      $done({ body: JSON.stringify({ statuses: [], since_id: null, total_number: 0 }) });
      return;
    }
    
    let statuses = data.cards
      .map((card) => {
        // 安全处理card_group
        if (card.card_group && Array.isArray(card.card_group)) {
          return card.card_group;
        }
        return card;
      })
      .flat()
      .filter((card) => {
        // 过滤card_type为9的微博卡片，同时保留其他可能有效的卡片
        return card && card.card_type === 9 && card.mblog;
      })
      .map((card) => card.mblog)
      .filter((status) => status) // 过滤掉undefined/null
      .map((status) => (status.isTop ? { ...status, label: "置顶" } : status));
    
    let sinceId = data.cardlistInfo?.since_id || null;
    
    $done({ 
      body: JSON.stringify({ 
        statuses: statuses, 
        since_id: sinceId, 
        total_number: statuses.length || 100 
      }) 
    });
  } catch (e) {
    console.log("[VVebo修复] 解析错误: " + e.message);
    $done({ body: JSON.stringify({ statuses: [], since_id: null, total_number: 0 }) });
  }
} else {
  $done({});
}
