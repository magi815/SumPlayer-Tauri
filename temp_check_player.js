(function(){
var player = document.querySelector('.html5-video-player');
if(!player) return JSON.stringify({error:'no player'});
var video = player.querySelector('video');
var classes = player.className.split(/\s+/).filter(function(c){
  return c.indexOf('mode')!==-1 || c.indexOf('ad')!==-1 || c.indexOf('playing')!==-1 || c.indexOf('paused')!==-1 || c.indexOf('buffer')!==-1 || c.indexOf('unstarted')!==-1 || c.indexOf('ended')!==-1;
});
return JSON.stringify({
  url: location.pathname,
  playerClasses: classes,
  videoReady: video ? video.readyState : -1,
  videoPaused: video ? video.paused : null,
  videoCurrentTime: video ? video.currentTime : null,
  adShowing: !!document.querySelector('.ad-showing,.ad-interrupting'),
  buffering: player.classList.contains('buffering-mode'),
  unstarted: player.classList.contains('unstarted-mode'),
  loadedMode: player.classList.contains('loaded-mode')
});
})()