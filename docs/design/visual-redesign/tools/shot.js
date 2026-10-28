const {chromium}=require('playwright');
(async()=>{const b=await chromium.launch();const p=await b.newPage({viewport:{width:+process.argv[4]||1200,height:+process.argv[5]||800}});await p.goto('file://'+process.argv[2]);await p.waitForTimeout(300);await p.screenshot({path:process.argv[3],fullPage:true});await b.close()})()
