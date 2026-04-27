// 在浏览器控制台运行此脚本来调试

console.log('=== 调试脚本开始 ===');

// 1. 检查数据
console.log('surgeModules:', typeof surgeModules);
console.log('amplify_nexus存在:', !!surgeModules.amplify_nexus);
console.log('amplify_nexus items数量:', surgeModules.amplify_nexus?.items?.length);

// 2. 检查第一个模块
const firstModule = surgeModules.amplify_nexus?.items?.[0];
console.log('第一个模块:', firstModule);
console.log('第一个模块有URL:', !!firstModule?.url);

// 3. 检查DOM
const moduleList = document.getElementById('moduleList');
console.log('moduleList容器:', moduleList);
console.log('moduleList子元素数:', moduleList?.children?.length);

// 4. 检查amplify_nexus分类
const amplifyCategory = Array.from(moduleList?.children || []).find(el => 
    el.textContent.includes('Amplify Nexus')
);
console.log('找到Amplify Nexus分类:', !!amplifyCategory);

if (amplifyCategory) {
    const modulesContainer = amplifyCategory.querySelector('.modules');
    console.log('模块容器:', modulesContainer);
    console.log('模块容器子元素数:', modulesContainer?.children?.length);
    
    // 检查第一个模块
    const firstModuleEl = modulesContainer?.children?.[0];
    console.log('第一个模块元素:', firstModuleEl);
    
    if (firstModuleEl) {
        const copyBtn = firstModuleEl.querySelector('.copy-btn');
        console.log('找到复制按钮:', !!copyBtn);
        console.log('按钮文本:', copyBtn?.textContent);
        console.log('按钮样式:', copyBtn ? window.getComputedStyle(copyBtn).display : 'N/A');
        console.log('按钮HTML:', firstModuleEl.innerHTML.substring(0, 500));
    }
}

// 5. 手动测试渲染一个模块
console.log('\n=== 手动测试渲染 ===');
const testDiv = document.createElement('div');
testDiv.style.cssText = 'position:fixed; top:10px; right:10px; background:rgba(0,0,0,0.9); color:#fff; padding:20px; z-index:9999; max-width:400px;';
testDiv.innerHTML = `
    <h3>测试模块</h3>
    <div style="display:flex; gap:10px; align-items:center; margin-top:10px;">
        <div style="flex:1;">
            <div><strong>测试模块名称</strong></div>
            <div style="font-size:0.9em; color:#aaa;">测试描述</div>
        </div>
        <button class="copy-btn" onclick="alert('按钮可点击!')">复制</button>
    </div>
`;
document.body.appendChild(testDiv);
console.log('已在右上角添加测试模块，检查按钮是否显示');

console.log('=== 调试脚本结束 ===');
