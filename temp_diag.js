(function(){
return JSON.stringify({
  url: location.pathname,
  adShowing: !!document.querySelector('.ad-showing,.ad-interrupting'),
  player: !!document.querySelector('.html5-video-player'),
  skipMsg: !!document.getElementById('__sp_skip_msg'),
  adskipFallback: window.__sp_adskip_fallback,
  iife_phase: window.__sp_iife_phase,
  sponsorScan: window.__sp_sponsor_scan
});
})()