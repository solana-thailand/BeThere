var Ra=Object.create;var It=Object.defineProperty;var Ea=Object.getOwnPropertyDescriptor;var ha=Object.getOwnPropertyNames;var Aa=Object.getPrototypeOf,Oa=Object.prototype.hasOwnProperty;var A=(t,e)=>()=>{try{return e||t((e={exports:{}}).exports,e),e.exports}catch(n){throw e=0,n}};var Na=(t,e,n,a)=>{if(e&&typeof e=="object"||typeof e=="function")for(let r of ha(e))!Oa.call(t,r)&&r!==n&&It(t,r,{get:()=>e[r],enumerable:!(a=Ea(e,r))||a.enumerable});return t};var Sa=(t,e,n)=>(n=t!=null?Ra(Aa(t)):{},Na(e||!t||!t.__esModule?It(n,"default",{value:t,enumerable:!0}):n,t));var sn=A((Au,on)=>{on.exports=function(){return typeof Promise=="function"&&Promise.prototype&&Promise.prototype.then}});var V=A(K=>{var tt,ud=[0,26,44,70,100,134,172,196,242,292,346,404,466,532,581,655,733,815,901,991,1085,1156,1258,1364,1474,1588,1706,1828,1921,2051,2185,2323,2465,2611,2761,2876,3034,3196,3362,3532,3706];K.getSymbolSize=function(e){if(!e)throw new Error('"version" cannot be null or undefined');if(e<1||e>40)throw new Error('"version" should be in range from 1 to 40');return e*4+17};K.getSymbolTotalCodewords=function(e){return ud[e]};K.getBCHDigit=function(t){let e=0;for(;t!==0;)e++,t>>>=1;return e};K.setToSJISFunction=function(e){if(typeof e!="function")throw new Error('"toSJISFunc" is not a valid function.');tt=e};K.isKanjiModeEnabled=function(){return typeof tt<"u"};K.toSJIS=function(e){return tt(e)}});var De=A(M=>{M.L={bit:1};M.M={bit:0};M.Q={bit:3};M.H={bit:2};function Rd(t){if(typeof t!="string")throw new Error("Param is not a string");switch(t.toLowerCase()){case"l":case"low":return M.L;case"m":case"medium":return M.M;case"q":case"quartile":return M.Q;case"h":case"high":return M.H;default:throw new Error("Unknown EC Level: "+t)}}M.isValid=function(e){return e&&typeof e.bit<"u"&&e.bit>=0&&e.bit<4};M.from=function(e,n){if(M.isValid(e))return e;try{return Rd(e)}catch{return n}}});var dn=A((Su,ln)=>{function cn(){this.buffer=[],this.length=0}cn.prototype={get:function(t){let e=Math.floor(t/8);return(this.buffer[e]>>>7-t%8&1)===1},put:function(t,e){for(let n=0;n<e;n++)this.putBit((t>>>e-n-1&1)===1)},getLengthInBits:function(){return this.length},putBit:function(t){let e=Math.floor(this.length/8);this.buffer.length<=e&&this.buffer.push(0),t&&(this.buffer[e]|=128>>>this.length%8),this.length++}};ln.exports=cn});var un=A((mu,_n)=>{function se(t){if(!t||t<1)throw new Error("BitMatrix size must be defined and greater than 0");this.size=t,this.data=new Uint8Array(t*t),this.reservedBit=new Uint8Array(t*t)}se.prototype.set=function(t,e,n,a){let r=t*this.size+e;this.data[r]=n,a&&(this.reservedBit[r]=!0)};se.prototype.get=function(t,e){return this.data[t*this.size+e]};se.prototype.xor=function(t,e,n){this.data[t*this.size+e]^=n};se.prototype.isReserved=function(t,e){return this.reservedBit[t*this.size+e]};_n.exports=se});var Rn=A(xe=>{var Ed=V().getSymbolSize;xe.getRowColCoords=function(e){if(e===1)return[];let n=Math.floor(e/7)+2,a=Ed(e),r=a===145?26:Math.ceil((a-13)/(2*n-2))*2,o=[a-7];for(let i=1;i<n-1;i++)o[i]=o[i-1]-r;return o.push(6),o.reverse()};xe.getPositions=function(e){let n=[],a=xe.getRowColCoords(e),r=a.length;for(let o=0;o<r;o++)for(let i=0;i<r;i++)o===0&&i===0||o===0&&i===r-1||o===r-1&&i===0||n.push([a[o],a[i]]);return n}});var An=A(hn=>{var hd=V().getSymbolSize,En=7;hn.getPositions=function(e){let n=hd(e);return[[0,0],[n-En,0],[0,n-En]]}});var On=A(m=>{m.Patterns={PATTERN000:0,PATTERN001:1,PATTERN010:2,PATTERN011:3,PATTERN100:4,PATTERN101:5,PATTERN110:6,PATTERN111:7};var W={N1:3,N2:3,N3:40,N4:10};m.isValid=function(e){return e!=null&&e!==""&&!isNaN(e)&&e>=0&&e<=7};m.from=function(e){return m.isValid(e)?parseInt(e,10):void 0};m.getPenaltyN1=function(e){let n=e.size,a=0,r=0,o=0,i=null,s=null;for(let c=0;c<n;c++){r=o=0,i=s=null;for(let l=0;l<n;l++){let d=e.get(c,l);d===i?r++:(r>=5&&(a+=W.N1+(r-5)),i=d,r=1),d=e.get(l,c),d===s?o++:(o>=5&&(a+=W.N1+(o-5)),s=d,o=1)}r>=5&&(a+=W.N1+(r-5)),o>=5&&(a+=W.N1+(o-5))}return a};m.getPenaltyN2=function(e){let n=e.size,a=0;for(let r=0;r<n-1;r++)for(let o=0;o<n-1;o++){let i=e.get(r,o)+e.get(r,o+1)+e.get(r+1,o)+e.get(r+1,o+1);(i===4||i===0)&&a++}return a*W.N2};m.getPenaltyN3=function(e){let n=e.size,a=0,r=0,o=0;for(let i=0;i<n;i++){r=o=0;for(let s=0;s<n;s++)r=r<<1&2047|e.get(i,s),s>=10&&(r===1488||r===93)&&a++,o=o<<1&2047|e.get(s,i),s>=10&&(o===1488||o===93)&&a++}return a*W.N3};m.getPenaltyN4=function(e){let n=0,a=e.data.length;for(let o=0;o<a;o++)n+=e.data[o];return Math.abs(Math.ceil(n*100/a/5)-10)*W.N4};function Ad(t,e,n){switch(t){case m.Patterns.PATTERN000:return(e+n)%2===0;case m.Patterns.PATTERN001:return e%2===0;case m.Patterns.PATTERN010:return n%3===0;case m.Patterns.PATTERN011:return(e+n)%3===0;case m.Patterns.PATTERN100:return(Math.floor(e/2)+Math.floor(n/3))%2===0;case m.Patterns.PATTERN101:return e*n%2+e*n%3===0;case m.Patterns.PATTERN110:return(e*n%2+e*n%3)%2===0;case m.Patterns.PATTERN111:return(e*n%3+(e+n)%2)%2===0;default:throw new Error("bad maskPattern:"+t)}}m.applyMask=function(e,n){let a=n.size;for(let r=0;r<a;r++)for(let o=0;o<a;o++)n.isReserved(o,r)||n.xor(o,r,Ad(e,o,r))};m.getBestMask=function(e,n){let a=Object.keys(m.Patterns).length,r=0,o=1/0;for(let i=0;i<a;i++){n(i),m.applyMask(i,e);let s=m.getPenaltyN1(e)+m.getPenaltyN2(e)+m.getPenaltyN3(e)+m.getPenaltyN4(e);m.applyMask(i,e),s<o&&(o=s,r=i)}return r}});var at=A(nt=>{var G=De(),Me=[1,1,1,1,1,1,1,1,1,1,2,2,1,2,2,4,1,2,4,4,2,4,4,4,2,4,6,5,2,4,6,6,2,5,8,8,4,5,8,8,4,5,8,11,4,8,10,11,4,9,12,16,4,9,16,16,6,10,12,18,6,10,17,16,6,11,16,19,6,13,18,21,7,14,21,25,8,16,20,25,8,17,23,25,9,17,23,34,9,18,25,30,10,20,27,32,12,21,29,35,12,23,34,37,12,25,34,40,13,26,35,42,14,28,38,45,15,29,40,48,16,31,43,51,17,33,45,54,18,35,48,57,19,37,51,60,19,38,53,63,20,40,56,66,21,43,59,70,22,45,62,74,24,47,65,77,25,49,68,81],Ue=[7,10,13,17,10,16,22,28,15,26,36,44,20,36,52,64,26,48,72,88,36,64,96,112,40,72,108,130,48,88,132,156,60,110,160,192,72,130,192,224,80,150,224,264,96,176,260,308,104,198,288,352,120,216,320,384,132,240,360,432,144,280,408,480,168,308,448,532,180,338,504,588,196,364,546,650,224,416,600,700,224,442,644,750,252,476,690,816,270,504,750,900,300,560,810,960,312,588,870,1050,336,644,952,1110,360,700,1020,1200,390,728,1050,1260,420,784,1140,1350,450,812,1200,1440,480,868,1290,1530,510,924,1350,1620,540,980,1440,1710,570,1036,1530,1800,570,1064,1590,1890,600,1120,1680,1980,630,1204,1770,2100,660,1260,1860,2220,720,1316,1950,2310,750,1372,2040,2430];nt.getBlocksCount=function(e,n){switch(n){case G.L:return Me[(e-1)*4+0];case G.M:return Me[(e-1)*4+1];case G.Q:return Me[(e-1)*4+2];case G.H:return Me[(e-1)*4+3];default:return}};nt.getTotalCodewordsCount=function(e,n){switch(n){case G.L:return Ue[(e-1)*4+0];case G.M:return Ue[(e-1)*4+1];case G.Q:return Ue[(e-1)*4+2];case G.H:return Ue[(e-1)*4+3];default:return}}});var Nn=A(Be=>{var ce=new Uint8Array(512),Pe=new Uint8Array(256);(function(){let e=1;for(let n=0;n<255;n++)ce[n]=e,Pe[e]=n,e<<=1,e&256&&(e^=285);for(let n=255;n<512;n++)ce[n]=ce[n-255]})();Be.log=function(e){if(e<1)throw new Error("log("+e+")");return Pe[e]};Be.exp=function(e){return ce[e]};Be.mul=function(e,n){return e===0||n===0?0:ce[Pe[e]+Pe[n]]}});var Sn=A(le=>{var rt=Nn();le.mul=function(e,n){let a=new Uint8Array(e.length+n.length-1);for(let r=0;r<e.length;r++)for(let o=0;o<n.length;o++)a[r+o]^=rt.mul(e[r],n[o]);return a};le.mod=function(e,n){let a=new Uint8Array(e);for(;a.length-n.length>=0;){let r=a[0];for(let i=0;i<n.length;i++)a[i]^=rt.mul(n[i],r);let o=0;for(;o<a.length&&a[o]===0;)o++;a=a.slice(o)}return a};le.generateECPolynomial=function(e){let n=new Uint8Array([1]);for(let a=0;a<e;a++)n=le.mul(n,new Uint8Array([1,rt.exp(a)]));return n}});var gn=A((wu,pn)=>{var mn=Sn();function ot(t){this.genPoly=void 0,this.degree=t,this.degree&&this.initialize(this.degree)}ot.prototype.initialize=function(e){this.degree=e,this.genPoly=mn.generateECPolynomial(this.degree)};ot.prototype.encode=function(e){if(!this.genPoly)throw new Error("Encoder not initialized");let n=new Uint8Array(e.length+this.degree);n.set(e);let a=mn.mod(n,this.genPoly),r=this.degree-a.length;if(r>0){let o=new Uint8Array(this.degree);return o.set(a,r),o}return a};pn.exports=ot});var it=A(fn=>{fn.isValid=function(e){return!isNaN(e)&&e>=1&&e<=40}});var st=A(B=>{var Tn="[0-9]+",Od="[A-Z $%*+\\-./:]+",de="(?:[u3000-u303F]|[u3040-u309F]|[u30A0-u30FF]|[uFF00-uFFEF]|[u4E00-u9FAF]|[u2605-u2606]|[u2190-u2195]|u203B|[u2010u2015u2018u2019u2025u2026u201Cu201Du2225u2260]|[u0391-u0451]|[u00A7u00A8u00B1u00B4u00D7u00F7])+";de=de.replace(/u/g,"\\u");var Nd="(?:(?![A-Z0-9 $%*+\\-./:]|"+de+`)(?:.|[\r
]))+`;B.KANJI=new RegExp(de,"g");B.BYTE_KANJI=new RegExp("[^A-Z0-9 $%*+\\-./:]+","g");B.BYTE=new RegExp(Nd,"g");B.NUMERIC=new RegExp(Tn,"g");B.ALPHANUMERIC=new RegExp(Od,"g");var Sd=new RegExp("^"+de+"$"),md=new RegExp("^"+Tn+"$"),pd=new RegExp("^[A-Z0-9 $%*+\\-./:]+$");B.testKanji=function(e){return Sd.test(e)};B.testNumeric=function(e){return md.test(e)};B.testAlphanumeric=function(e){return pd.test(e)}});var z=A(f=>{var gd=it(),ct=st();f.NUMERIC={id:"Numeric",bit:1,ccBits:[10,12,14]};f.ALPHANUMERIC={id:"Alphanumeric",bit:2,ccBits:[9,11,13]};f.BYTE={id:"Byte",bit:4,ccBits:[8,16,16]};f.KANJI={id:"Kanji",bit:8,ccBits:[8,10,12]};f.MIXED={bit:-1};f.getCharCountIndicator=function(e,n){if(!e.ccBits)throw new Error("Invalid mode: "+e);if(!gd.isValid(n))throw new Error("Invalid version: "+n);return n>=1&&n<10?e.ccBits[0]:n<27?e.ccBits[1]:e.ccBits[2]};f.getBestModeForData=function(e){return ct.testNumeric(e)?f.NUMERIC:ct.testAlphanumeric(e)?f.ALPHANUMERIC:ct.testKanji(e)?f.KANJI:f.BYTE};f.toString=function(e){if(e&&e.id)return e.id;throw new Error("Invalid mode")};f.isValid=function(e){return e&&e.bit&&e.ccBits};function fd(t){if(typeof t!="string")throw new Error("Param is not a string");switch(t.toLowerCase()){case"numeric":return f.NUMERIC;case"alphanumeric":return f.ALPHANUMERIC;case"kanji":return f.KANJI;case"byte":return f.BYTE;default:throw new Error("Unknown mode: "+t)}}f.from=function(e,n){if(f.isValid(e))return e;try{return fd(e)}catch{return n}}});var bn=A(q=>{var Fe=V(),Td=at(),In=De(),H=z(),lt=it(),wn=7973,Cn=Fe.getBCHDigit(wn);function Id(t,e,n){for(let a=1;a<=40;a++)if(e<=q.getCapacity(a,n,t))return a}function Ln(t,e){return H.getCharCountIndicator(t,e)+4}function Cd(t,e){let n=0;return t.forEach(function(a){let r=Ln(a.mode,e);n+=r+a.getBitsLength()}),n}function wd(t,e){for(let n=1;n<=40;n++)if(Cd(t,n)<=q.getCapacity(n,e,H.MIXED))return n}q.from=function(e,n){return lt.isValid(e)?parseInt(e,10):n};q.getCapacity=function(e,n,a){if(!lt.isValid(e))throw new Error("Invalid QR Code version");typeof a>"u"&&(a=H.BYTE);let r=Fe.getSymbolTotalCodewords(e),o=Td.getTotalCodewordsCount(e,n),i=(r-o)*8;if(a===H.MIXED)return i;let s=i-Ln(a,e);switch(a){case H.NUMERIC:return Math.floor(s/10*3);case H.ALPHANUMERIC:return Math.floor(s/11*2);case H.KANJI:return Math.floor(s/13);case H.BYTE:default:return Math.floor(s/8)}};q.getBestVersionForData=function(e,n){let a,r=In.from(n,In.M);if(Array.isArray(e)){if(e.length>1)return wd(e,r);if(e.length===0)return 1;a=e[0]}else a=e;return Id(a.mode,a.getLength(),r)};q.getEncodedBits=function(e){if(!lt.isValid(e)||e<7)throw new Error("Invalid QR Code version");let n=e<<12;for(;Fe.getBCHDigit(n)-Cn>=0;)n^=wn<<Fe.getBCHDigit(n)-Cn;return e<<12|n}});var xn=A(Dn=>{var dt=V(),vn=1335,Ld=21522,yn=dt.getBCHDigit(vn);Dn.getEncodedBits=function(e,n){let a=e.bit<<3|n,r=a<<10;for(;dt.getBCHDigit(r)-yn>=0;)r^=vn<<dt.getBCHDigit(r)-yn;return(a<<10|r)^Ld}});var Un=A((xu,Mn)=>{var bd=z();function J(t){this.mode=bd.NUMERIC,this.data=t.toString()}J.getBitsLength=function(e){return 10*Math.floor(e/3)+(e%3?e%3*3+1:0)};J.prototype.getLength=function(){return this.data.length};J.prototype.getBitsLength=function(){return J.getBitsLength(this.data.length)};J.prototype.write=function(e){let n,a,r;for(n=0;n+3<=this.data.length;n+=3)a=this.data.substr(n,3),r=parseInt(a,10),e.put(r,10);let o=this.data.length-n;o>0&&(a=this.data.substr(n),r=parseInt(a,10),e.put(r,o*3+1))};Mn.exports=J});var Bn=A((Mu,Pn)=>{var yd=z(),_t=["0","1","2","3","4","5","6","7","8","9","A","B","C","D","E","F","G","H","I","J","K","L","M","N","O","P","Q","R","S","T","U","V","W","X","Y","Z"," ","$","%","*","+","-",".","/",":"];function Z(t){this.mode=yd.ALPHANUMERIC,this.data=t}Z.getBitsLength=function(e){return 11*Math.floor(e/2)+6*(e%2)};Z.prototype.getLength=function(){return this.data.length};Z.prototype.getBitsLength=function(){return Z.getBitsLength(this.data.length)};Z.prototype.write=function(e){let n;for(n=0;n+2<=this.data.length;n+=2){let a=_t.indexOf(this.data[n])*45;a+=_t.indexOf(this.data[n+1]),e.put(a,11)}this.data.length%2&&e.put(_t.indexOf(this.data[n]),6)};Pn.exports=Z});var $n=A((Uu,Fn)=>{var vd=z();function Q(t){this.mode=vd.BYTE,typeof t=="string"?this.data=new TextEncoder().encode(t):this.data=new Uint8Array(t)}Q.getBitsLength=function(e){return e*8};Q.prototype.getLength=function(){return this.data.length};Q.prototype.getBitsLength=function(){return Q.getBitsLength(this.data.length)};Q.prototype.write=function(t){for(let e=0,n=this.data.length;e<n;e++)t.put(this.data[e],8)};Fn.exports=Q});var Vn=A((Pu,kn)=>{var Dd=z(),xd=V();function ee(t){this.mode=Dd.KANJI,this.data=t}ee.getBitsLength=function(e){return e*13};ee.prototype.getLength=function(){return this.data.length};ee.prototype.getBitsLength=function(){return ee.getBitsLength(this.data.length)};ee.prototype.write=function(t){let e;for(e=0;e<this.data.length;e++){let n=xd.toSJIS(this.data[e]);if(n>=33088&&n<=40956)n-=33088;else if(n>=57408&&n<=60351)n-=49472;else throw new Error("Invalid SJIS character: "+this.data[e]+`
Make sure your charset is UTF-8`);n=(n>>>8&255)*192+(n&255),t.put(n,13)}};kn.exports=ee});var Gn=A((Bu,ut)=>{"use strict";var _e={single_source_shortest_paths:function(t,e,n){var a={},r={};r[e]=0;var o=_e.PriorityQueue.make();o.push(e,0);for(var i,s,c,l,d,N,E,T,b;!o.empty();){i=o.pop(),s=i.value,l=i.cost,d=t[s]||{};for(c in d)d.hasOwnProperty(c)&&(N=d[c],E=l+N,T=r[c],b=typeof r[c]>"u",(b||T>E)&&(r[c]=E,o.push(c,E),a[c]=s))}if(typeof n<"u"&&typeof r[n]>"u"){var p=["Could not find a path from ",e," to ",n,"."].join("");throw new Error(p)}return a},extract_shortest_path_from_predecessor_list:function(t,e){for(var n=[],a=e,r;a;)n.push(a),r=t[a],a=t[a];return n.reverse(),n},find_path:function(t,e,n){var a=_e.single_source_shortest_paths(t,e,n);return _e.extract_shortest_path_from_predecessor_list(a,n)},PriorityQueue:{make:function(t){var e=_e.PriorityQueue,n={},a;t=t||{};for(a in e)e.hasOwnProperty(a)&&(n[a]=e[a]);return n.queue=[],n.sorter=t.sorter||e.default_sorter,n},default_sorter:function(t,e){return t.cost-e.cost},push:function(t,e){var n={value:t,cost:e};this.queue.push(n),this.queue.sort(this.sorter)},pop:function(){return this.queue.shift()},empty:function(){return this.queue.length===0}}};typeof ut<"u"&&(ut.exports=_e)});var jn=A(te=>{var h=z(),Kn=Un(),Wn=Bn(),qn=$n(),Yn=Vn(),ue=st(),$e=V(),Md=Gn();function zn(t){return unescape(encodeURIComponent(t)).length}function Re(t,e,n){let a=[],r;for(;(r=t.exec(n))!==null;)a.push({data:r[0],index:r.index,mode:e,length:r[0].length});return a}function Xn(t){let e=Re(ue.NUMERIC,h.NUMERIC,t),n=Re(ue.ALPHANUMERIC,h.ALPHANUMERIC,t),a,r;return $e.isKanjiModeEnabled()?(a=Re(ue.BYTE,h.BYTE,t),r=Re(ue.KANJI,h.KANJI,t)):(a=Re(ue.BYTE_KANJI,h.BYTE,t),r=[]),e.concat(n,a,r).sort(function(i,s){return i.index-s.index}).map(function(i){return{data:i.data,mode:i.mode,length:i.length}})}function Rt(t,e){switch(e){case h.NUMERIC:return Kn.getBitsLength(t);case h.ALPHANUMERIC:return Wn.getBitsLength(t);case h.KANJI:return Yn.getBitsLength(t);case h.BYTE:return qn.getBitsLength(t)}}function Ud(t){return t.reduce(function(e,n){let a=e.length-1>=0?e[e.length-1]:null;return a&&a.mode===n.mode?(e[e.length-1].data+=n.data,e):(e.push(n),e)},[])}function Pd(t){let e=[];for(let n=0;n<t.length;n++){let a=t[n];switch(a.mode){case h.NUMERIC:e.push([a,{data:a.data,mode:h.ALPHANUMERIC,length:a.length},{data:a.data,mode:h.BYTE,length:a.length}]);break;case h.ALPHANUMERIC:e.push([a,{data:a.data,mode:h.BYTE,length:a.length}]);break;case h.KANJI:e.push([a,{data:a.data,mode:h.BYTE,length:zn(a.data)}]);break;case h.BYTE:e.push([{data:a.data,mode:h.BYTE,length:zn(a.data)}])}}return e}function Bd(t,e){let n={},a={start:{}},r=["start"];for(let o=0;o<t.length;o++){let i=t[o],s=[];for(let c=0;c<i.length;c++){let l=i[c],d=""+o+c;s.push(d),n[d]={node:l,lastCount:0},a[d]={};for(let N=0;N<r.length;N++){let E=r[N];n[E]&&n[E].node.mode===l.mode?(a[E][d]=Rt(n[E].lastCount+l.length,l.mode)-Rt(n[E].lastCount,l.mode),n[E].lastCount+=l.length):(n[E]&&(n[E].lastCount=l.length),a[E][d]=Rt(l.length,l.mode)+4+h.getCharCountIndicator(l.mode,e))}}r=s}for(let o=0;o<r.length;o++)a[r[o]].end=0;return{map:a,table:n}}function Hn(t,e){let n,a=h.getBestModeForData(t);if(n=h.from(e,a),n!==h.BYTE&&n.bit<a.bit)throw new Error('"'+t+'" cannot be encoded with mode '+h.toString(n)+`.
 Suggested mode is: `+h.toString(a));switch(n===h.KANJI&&!$e.isKanjiModeEnabled()&&(n=h.BYTE),n){case h.NUMERIC:return new Kn(t);case h.ALPHANUMERIC:return new Wn(t);case h.KANJI:return new Yn(t);case h.BYTE:return new qn(t)}}te.fromArray=function(e){return e.reduce(function(n,a){return typeof a=="string"?n.push(Hn(a,null)):a.data&&n.push(Hn(a.data,a.mode)),n},[])};te.fromString=function(e,n){let a=Xn(e,$e.isKanjiModeEnabled()),r=Pd(a),o=Bd(r,n),i=Md.find_path(o.map,"start","end"),s=[];for(let c=1;c<i.length-1;c++)s.push(o.table[i[c]].node);return te.fromArray(Ud(s))};te.rawSplit=function(e){return te.fromArray(Xn(e,$e.isKanjiModeEnabled()))}});var Zn=A(Jn=>{var Ve=V(),Et=De(),Fd=dn(),$d=un(),kd=Rn(),Vd=An(),Ot=On(),Nt=at(),Gd=gn(),ke=bn(),zd=xn(),Hd=z(),ht=jn();function Kd(t,e){let n=t.size,a=Vd.getPositions(e);for(let r=0;r<a.length;r++){let o=a[r][0],i=a[r][1];for(let s=-1;s<=7;s++)if(!(o+s<=-1||n<=o+s))for(let c=-1;c<=7;c++)i+c<=-1||n<=i+c||(s>=0&&s<=6&&(c===0||c===6)||c>=0&&c<=6&&(s===0||s===6)||s>=2&&s<=4&&c>=2&&c<=4?t.set(o+s,i+c,!0,!0):t.set(o+s,i+c,!1,!0))}}function Wd(t){let e=t.size;for(let n=8;n<e-8;n++){let a=n%2===0;t.set(n,6,a,!0),t.set(6,n,a,!0)}}function qd(t,e){let n=kd.getPositions(e);for(let a=0;a<n.length;a++){let r=n[a][0],o=n[a][1];for(let i=-2;i<=2;i++)for(let s=-2;s<=2;s++)i===-2||i===2||s===-2||s===2||i===0&&s===0?t.set(r+i,o+s,!0,!0):t.set(r+i,o+s,!1,!0)}}function Yd(t,e){let n=t.size,a=ke.getEncodedBits(e),r,o,i;for(let s=0;s<18;s++)r=Math.floor(s/3),o=s%3+n-8-3,i=(a>>s&1)===1,t.set(r,o,i,!0),t.set(o,r,i,!0)}function At(t,e,n){let a=t.size,r=zd.getEncodedBits(e,n),o,i;for(o=0;o<15;o++)i=(r>>o&1)===1,o<6?t.set(o,8,i,!0):o<8?t.set(o+1,8,i,!0):t.set(a-15+o,8,i,!0),o<8?t.set(8,a-o-1,i,!0):o<9?t.set(8,15-o-1+1,i,!0):t.set(8,15-o-1,i,!0);t.set(a-8,8,1,!0)}function Xd(t,e){let n=t.size,a=-1,r=n-1,o=7,i=0;for(let s=n-1;s>0;s-=2)for(s===6&&s--;;){for(let c=0;c<2;c++)if(!t.isReserved(r,s-c)){let l=!1;i<e.length&&(l=(e[i]>>>o&1)===1),t.set(r,s-c,l),o--,o===-1&&(i++,o=7)}if(r+=a,r<0||n<=r){r-=a,a=-a;break}}}function jd(t,e,n){let a=new Fd;n.forEach(function(c){a.put(c.mode.bit,4),a.put(c.getLength(),Hd.getCharCountIndicator(c.mode,t)),c.write(a)});let r=Ve.getSymbolTotalCodewords(t),o=Nt.getTotalCodewordsCount(t,e),i=(r-o)*8;for(a.getLengthInBits()+4<=i&&a.put(0,4);a.getLengthInBits()%8!==0;)a.putBit(0);let s=(i-a.getLengthInBits())/8;for(let c=0;c<s;c++)a.put(c%2?17:236,8);return Jd(a,t,e)}function Jd(t,e,n){let a=Ve.getSymbolTotalCodewords(e),r=Nt.getTotalCodewordsCount(e,n),o=a-r,i=Nt.getBlocksCount(e,n),s=a%i,c=i-s,l=Math.floor(a/i),d=Math.floor(o/i),N=d+1,E=l-d,T=new Gd(E),b=0,p=new Array(i),L=new Array(i),C=0,v=new Uint8Array(t.buffer);for(let R=0;R<i;R++){let g=R<c?d:N;p[R]=v.slice(b,b+g),L[R]=T.encode(p[R]),b+=g,C=Math.max(C,g)}let U=new Uint8Array(a),y=0,_,u;for(_=0;_<C;_++)for(u=0;u<i;u++)_<p[u].length&&(U[y++]=p[u][_]);for(_=0;_<E;_++)for(u=0;u<i;u++)U[y++]=L[u][_];return U}function Zd(t,e,n,a){let r;if(Array.isArray(t))r=ht.fromArray(t);else if(typeof t=="string"){let l=e;if(!l){let d=ht.rawSplit(t);l=ke.getBestVersionForData(d,n)}r=ht.fromString(t,l||40)}else throw new Error("Invalid data");let o=ke.getBestVersionForData(r,n);if(!o)throw new Error("The amount of data is too big to be stored in a QR Code");if(!e)e=o;else if(e<o)throw new Error(`
The chosen QR Code version cannot contain this amount of data.
Minimum version required to store current data is: `+o+`.
`);let i=jd(e,n,r),s=Ve.getSymbolSize(e),c=new $d(s);return Kd(c,e),Wd(c),qd(c,e),At(c,n,0),e>=7&&Yd(c,e),Xd(c,i),isNaN(a)&&(a=Ot.getBestMask(c,At.bind(null,c,n))),Ot.applyMask(a,c),At(c,n,a),{modules:c,version:e,errorCorrectionLevel:n,maskPattern:a,segments:r}}Jn.create=function(e,n){if(typeof e>"u"||e==="")throw new Error("No input text");let a=Et.M,r,o;return typeof n<"u"&&(a=Et.from(n.errorCorrectionLevel,Et.M),r=ke.from(n.version),o=Ot.from(n.maskPattern),n.toSJISFunc&&Ve.setToSJISFunction(n.toSJISFunc)),Zd(e,r,a,o)}});var St=A(Y=>{function Qn(t){if(typeof t=="number"&&(t=t.toString()),typeof t!="string")throw new Error("Color should be defined as hex string");let e=t.slice().replace("#","").split("");if(e.length<3||e.length===5||e.length>8)throw new Error("Invalid hex color: "+t);(e.length===3||e.length===4)&&(e=Array.prototype.concat.apply([],e.map(function(a){return[a,a]}))),e.length===6&&e.push("F","F");let n=parseInt(e.join(""),16);return{r:n>>24&255,g:n>>16&255,b:n>>8&255,a:n&255,hex:"#"+e.slice(0,6).join("")}}Y.getOptions=function(e){e||(e={}),e.color||(e.color={});let n=typeof e.margin>"u"||e.margin===null||e.margin<0?4:e.margin,a=e.width&&e.width>=21?e.width:void 0,r=e.scale||4;return{width:a,scale:a?4:r,margin:n,color:{dark:Qn(e.color.dark||"#000000ff"),light:Qn(e.color.light||"#ffffffff")},type:e.type,rendererOpts:e.rendererOpts||{}}};Y.getScale=function(e,n){return n.width&&n.width>=e+n.margin*2?n.width/(e+n.margin*2):n.scale};Y.getImageWidth=function(e,n){let a=Y.getScale(e,n);return Math.floor((e+n.margin*2)*a)};Y.qrToImageData=function(e,n,a){let r=n.modules.size,o=n.modules.data,i=Y.getScale(r,a),s=Math.floor((r+a.margin*2)*i),c=a.margin*i,l=[a.color.light,a.color.dark];for(let d=0;d<s;d++)for(let N=0;N<s;N++){let E=(d*s+N)*4,T=a.color.light;if(d>=c&&N>=c&&d<s-c&&N<s-c){let b=Math.floor((d-c)/i),p=Math.floor((N-c)/i);T=l[o[b*r+p]?1:0]}e[E++]=T.r,e[E++]=T.g,e[E++]=T.b,e[E]=T.a}}});var ea=A(Ge=>{var mt=St();function Qd(t,e,n){t.clearRect(0,0,e.width,e.height),e.style||(e.style={}),e.height=n,e.width=n,e.style.height=n+"px",e.style.width=n+"px"}function e_(){try{return document.createElement("canvas")}catch{throw new Error("You need to specify a canvas element")}}Ge.render=function(e,n,a){let r=a,o=n;typeof r>"u"&&(!n||!n.getContext)&&(r=n,n=void 0),n||(o=e_()),r=mt.getOptions(r);let i=mt.getImageWidth(e.modules.size,r),s=o.getContext("2d"),c=s.createImageData(i,i);return mt.qrToImageData(c.data,e,r),Qd(s,o,i),s.putImageData(c,0,0),o};Ge.renderToDataURL=function(e,n,a){let r=a;typeof r>"u"&&(!n||!n.getContext)&&(r=n,n=void 0),r||(r={});let o=Ge.render(e,n,r),i=r.type||"image/png",s=r.rendererOpts||{};return o.toDataURL(i,s.quality)}});var aa=A(na=>{var t_=St();function ta(t,e){let n=t.a/255,a=e+'="'+t.hex+'"';return n<1?a+" "+e+'-opacity="'+n.toFixed(2).slice(1)+'"':a}function pt(t,e,n){let a=t+e;return typeof n<"u"&&(a+=" "+n),a}function n_(t,e,n){let a="",r=0,o=!1,i=0;for(let s=0;s<t.length;s++){let c=Math.floor(s%e),l=Math.floor(s/e);!c&&!o&&(o=!0),t[s]?(i++,s>0&&c>0&&t[s-1]||(a+=o?pt("M",c+n,.5+l+n):pt("m",r,0),r=0,o=!1),c+1<e&&t[s+1]||(a+=pt("h",i),i=0)):r++}return a}na.render=function(e,n,a){let r=t_.getOptions(n),o=e.modules.size,i=e.modules.data,s=o+r.margin*2,c=r.color.light.a?"<path "+ta(r.color.light,"fill")+' d="M0 0h'+s+"v"+s+'H0z"/>':"",l="<path "+ta(r.color.dark,"stroke")+' d="'+n_(i,o,r.margin)+'"/>',d='viewBox="0 0 '+s+" "+s+'"',E='<svg xmlns="http://www.w3.org/2000/svg" '+(r.width?'width="'+r.width+'" height="'+r.width+'" ':"")+d+' shape-rendering="crispEdges">'+c+l+`</svg>
`;return typeof a=="function"&&a(null,E),E}});var oa=A(Ee=>{var a_=sn(),gt=Zn(),ra=ea(),r_=aa();function ft(t,e,n,a,r){let o=[].slice.call(arguments,1),i=o.length,s=typeof o[i-1]=="function";if(!s&&!a_())throw new Error("Callback required as last argument");if(s){if(i<2)throw new Error("Too few arguments provided");i===2?(r=n,n=e,e=a=void 0):i===3&&(e.getContext&&typeof r>"u"?(r=a,a=void 0):(r=a,a=n,n=e,e=void 0))}else{if(i<1)throw new Error("Too few arguments provided");return i===1?(n=e,e=a=void 0):i===2&&!e.getContext&&(a=n,n=e,e=void 0),new Promise(function(c,l){try{let d=gt.create(n,a);c(t(d,e,a))}catch(d){l(d)}})}try{let c=gt.create(n,a);r(null,t(c,e,a))}catch(c){r(c)}}Ee.create=gt.create;Ee.toCanvas=ft.bind(null,ra.render);Ee.toDataURL=ft.bind(null,ra.renderToDataURL);Ee.toString=ft.bind(null,function(t,e,n){return r_.render(t,n)})});var ma=1,pa=2,ga=3,fa=4,Ta=5,Ia=6,Ca=7,wa=8,La=9,ba=10,ya=11,va=12,Da=-32700,xa=-32603,Ma=-32602,Ua=-32601,Pa=-32600,Ba=-32021,Fa=-32020,$a=-32019,ka=-32018,Va=-32017,Ga=-32016,za=-32015,Ha=-32014,Ka=-32013,Wa=-32012,qa=-32011,Ya=-32010,Xa=-32009,ja=-32008,Ja=-32007,Za=-32006,Qa=-32005,er=-32004,tr=-32003,nr=-32002,ar=-32001,rr=28e5,or=2800001,ir=2800002,sr=2800003,cr=2800004,lr=2800005,dr=2800006,_r=2800007,ur=2800008,Rr=2800009,Er=2800010,hr=2800011,Ar=323e4,Or=32300001,Nr=3230002,Sr=3230003,mr=3230004,pr=361e4,gr=3610001,fr=3610002,Tr=3610003,Ir=3610004,Cr=3610005,wr=3610006,Lr=3610007,br=3611e3,yr=3704e3,vr=3704001,Dr=3704002,xr=3704003,Mr=3704004,Ur=3704005,Pr=3704006,Br=3712e3,Fr=4128e3,$r=4128001,kr=4128002,Vr=4615e3,Gr=4615001,zr=4615002,Hr=4615003,Kr=4615004,Wr=4615005,qr=4615006,Yr=4615007,Xr=4615008,jr=4615009,Jr=4615010,Zr=4615011,Qr=4615012,eo=4615013,to=4615014,no=4615015,ao=4615016,ro=4615017,oo=4615018,io=4615019,so=4615020,co=4615021,lo=4615022,_o=4615023,uo=4615024,Ro=4615025,Eo=4615026,ho=4615027,Ao=4615028,Oo=4615029,No=4615030,So=4615031,mo=4615032,po=4615033,go=4615034,fo=4615035,To=4615036,Io=4615037,Co=4615038,wo=4615039,Lo=4615040,bo=4615041,yo=4615042,vo=4615043,Do=4615044,xo=4615045,Mo=4615046,Uo=4615047,Po=4615048,Bo=4615049,Fo=4615050,$o=4615051,ko=4615052,Vo=4615053,Go=4615054,zo=5508e3,Ho=5508001,Ko=5508002,Wo=5508003,qo=5508004,Yo=5508005,Xo=5508006,jo=5508007,Jo=5508008,Zo=5508009,Qo=5508010,ei=5508011,ti=5508012,ni=5607e3,ai=5607001,ri=5607002,oi=5607003,ii=5607004,si=5607005,ci=5607006,li=5607007,di=5607008,_i=5607009,ui=5607010,Ri=5607011,Ei=5607012,hi=5607013,Ai=5607014,Oi=5607015,Ni=5607016,Si=5607017,mi=5607018,pi=5607019,gi=5663e3,fi=5663001,Ti=5663002,Ii=5663003,Ci=5663004,wi=5663005,Li=5663006,bi=5663007,yi=5663008,vi=5663009,Di=5663010,xi=5663011,Mi=5663012,Ui=5663013,Pi=5663014,Bi=5663015,Fi=5663016,$i=5663017,ki=5663018,Vi=5663019,Gi=5663020,zi=5663021,Hi=5663022,Ki=5663023,Wi=5663024,qi=5663025,Yi=5663026,Xi=5663027,ji=5663028,Ji=5663029,Zi=5663030,Qi=5663031,es=5663032,ts=5663033,ns=5663034,as=5663035,rs=5663036,os=5663037,is=5663038,ss=5664e3,cs=5664001,ls=705e4,ds=7050001,_s=7050002,us=7050003,Rs=7050004,Es=7050005,hs=7050006,As=7050007,Os=7050008,Ns=7050009,Ss=7050010,ms=7050011,ps=7050012,gs=7050013,fs=7050014,Ts=7050015,Is=7050016,Cs=7050017,ws=7050018,Ls=7050019,bs=7050020,ys=7050021,vs=7050022,Ds=7050023,xs=7050024,Ms=7050025,Us=7050026,Ps=7050027,Bs=7050028,Fs=7050029,$s=7050030,ks=7050031,Vs=7050032,Gs=7050033,zs=7050034,Hs=7050035,Ks=7050036,Ws=7618e3,qs=7618001,Ys=7618002,Xs=7618003,js=7618004,Js=7618005,Zs=7618006,Qs=7618007,ec=7618008,tc=7618009,nc=7618010,ac=7618011,rc=8078e3,oc=8078001,ic=8078002,sc=8078003,cc=8078004,lc=8078005,dc=8078006,_c=8078007,uc=8078008,Rc=8078009,Ec=8078010,hc=8078011,oe=8078012,Ac=8078013,Oc=8078014,Nc=8078015,Sc=8078016,mc=8078017,pc=8078018,gc=8078019,fc=8078020,Tc=8078021,Ic=8078022,Cc=8078023,wc=8078024,Lc=8078025,bc=809e4,yc=8090001,vc=8090002,Dc=8090003,xc=8090004,Mc=8090005,Uc=8090006,Pc=8090007,Bc=8090008,Fc=8090009,$c=8090010,kc=8090011,Vc=8090012,Gc=81e5,zc=8100001,Hc=8100002,Kc=8100003,Wc=819e4,qc=8190001,Yc=8190002,Xc=8190003,jc=8190004,Jc=8195e3,Zc=8195001,Qc=85e5,el=8500001,tl=8500002,nl=8500003,al=8500004,rl=8500005,ol=8500006,il=89e5,sl=8900001,cl=8900002,ll=8900003,dl=9e6,_l=9000001,ul=9000002,Rl=99e5,El=9900001,hl=9900002,Al=9900003,Ol=9900004,Nl=9900005,Sl=9900006;function Ct(t){return Array.isArray(t)?"%5B"+t.map(Ct).join("%2C%20")+"%5D":typeof t=="bigint"?`${t}n`:encodeURIComponent(String(t!=null&&Object.getPrototypeOf(t)===null?{...t}:t))}function ml([t,e]){return`${t}=${Ct(e)}`}function pl(t){let e=Object.entries(t).map(ml).join("&");return btoa(e)}var $_={[Ar]:"Account not found at address: $address",[mr]:"Not all accounts were decoded. Encoded accounts found at addresses: $addresses.",[Sr]:"Expected decoded account at address: $address",[Nr]:"Failed to decode account data at address: $address",[Or]:"Accounts not found at addresses: $addresses",[Rr]:"Unable to find a viable program address bump seed.",[ir]:"$putativeAddress is not a base58-encoded address.",[rr]:"Expected base58 encoded address to decode to a byte array of length 32. Actual length: $actualLength.",[sr]:"The `CryptoKey` must be an `Ed25519` public key.",[hr]:"$putativeOffCurveAddress is not a base58-encoded off-curve address.",[ur]:"Invalid seeds; point must fall off the Ed25519 curve.",[cr]:"Expected given program derived address to have the following format: [Address, ProgramDerivedAddressBump].",[dr]:"A maximum of $maxSeeds seeds, including the bump seed, may be supplied when creating an address. Received: $actual.",[_r]:"The seed at index $index with length $actual exceeds the maximum length of $maxSeedLength bytes.",[lr]:"Expected program derived address bump to be in the range [0, 255], got: $bump.",[Er]:"Program address cannot end with PDA marker.",[or]:"Expected base58-encoded address string of length in the range [32, 44]. Actual length: $actualLength.",[fa]:"Expected base58-encoded blockhash string of length in the range [32, 44]. Actual length: $actualLength.",[ma]:"The network has progressed past the last block for which this transaction could have been committed.",[rc]:"Codec [$codecDescription] cannot decode empty byte arrays.",[Ic]:"Enum codec cannot use lexical values [$stringValues] as discriminators. Either remove all lexical values or set `useValuesAsDiscriminators` to `false`.",[fc]:"Sentinel [$hexSentinel] must not be present in encoded bytes [$hexEncodedBytes].",[lc]:"Encoder and decoder must have the same fixed size, got [$encoderFixedSize] and [$decoderFixedSize].",[dc]:"Encoder and decoder must have the same max size, got [$encoderMaxSize] and [$decoderMaxSize].",[cc]:"Encoder and decoder must either both be fixed-size or variable-size.",[uc]:"Enum discriminator out of range. Expected a number in [$formattedValidDiscriminators], got $discriminator.",[ic]:"Expected a fixed-size codec, got a variable-size one.",[Ac]:"Codec [$codecDescription] expected a positive byte length, got $bytesLength.",[sc]:"Expected a variable-size codec, got a fixed-size one.",[gc]:"Codec [$codecDescription] expected zero-value [$hexZeroValue] to have the same size as the provided fixed-size item [$expectedSize bytes].",[oc]:"Codec [$codecDescription] expected $expected bytes, got $bytesLength.",[pc]:"Expected byte array constant [$hexConstant] to be present in data [$hexData] at offset [$offset].",[Rc]:"Invalid discriminated union variant. Expected one of [$variants], got $value.",[Ec]:"Invalid enum variant. Expected one of [$stringValues] or a number in [$formattedNumericalValues], got $variant.",[Nc]:"Invalid literal union variant. Expected one of [$variants], got $value.",[_c]:"Expected [$codecDescription] to have $expected items, got $actual.",[oe]:"Invalid value $value for base $base with alphabet $alphabet.",[Sc]:"Literal union discriminator out of range. Expected a number between $minRange and $maxRange, got $discriminator.",[hc]:"Codec [$codecDescription] expected number to be in the range [$min, $max], got $value.",[Oc]:"Codec [$codecDescription] expected offset to be in the range [0, $bytesLength], got $offset.",[Tc]:"Expected sentinel [$hexSentinel] to be present in decoded bytes [$hexDecodedBytes].",[mc]:"Union variant out of range. Expected an index between $minRange and $maxRange, got $variant.",[Cc]:"This decoder expected a byte array of exactly $expectedLength bytes, but $numExcessBytes unexpected excess bytes remained after decoding. Are you sure that you have chosen the correct decoder for this data?",[wc]:"Invalid pattern match value. The provided value does not match any of the specified patterns.",[Lc]:"Invalid pattern match bytes. The provided byte array does not match any of the specified patterns.",[br]:"No random values implementation could be found.",[ya]:"Failed to send transaction$causeMessage",[va]:"Failed to send transactions$causeMessages",[Pc]:"Fixed-point operation `$operation` of kind `$kind` overflowed. Expected a raw bigint in [$min, $max], got $result.",[Fc]:"Fixed-point division by zero for value of kind `$kind` ($signedness, $totalBits bits).",[Dc]:"`fractionalBits` ($fractionalBits) must not exceed `totalBits` ($totalBits).",[vc]:"Invalid `decimals`. Expected a non-negative integer, got $decimals.",[yc]:"Invalid `fractionalBits`. Expected a non-negative integer, got $fractionalBits.",[Mc]:"Invalid string `$input` for fixed-point value of kind `$kind`.",[bc]:"Invalid `totalBits`. Expected a positive integer, got $totalBits.",[Uc]:"Invalid ratio $numerator/$denominator for fixed-point value of kind `$kind`. Denominator must be non-zero.",[kc]:"Fixed-point value of kind `$kind` has a malformed `raw` field. Expected a bigint, got `$raw`.",[Bc]:"Fixed-point `$operation` operation expected $expectedKind ($expectedSignedness, $expectedTotalBits bits, $expectedScale $expectedScaleLabel); got $actualKind ($actualSignedness, $actualTotalBits bits, $actualScale $actualScaleLabel).",[$c]:"Fixed-point operation `$operation` of kind `$kind` cannot be performed exactly; pass a rounding mode other than `strict` to allow a rounded result.",[Vc]:"Fixed-point codec of kind `$kind` requires `totalBits` to be a multiple of 8; got $totalBits.",[xc]:"Fixed-point value of kind `$kind` is out of range for $signedness $totalBits-bit storage. Expected a raw bigint in [$min, $max], got $raw.",[Br]:"Filesystem operation `$operation` is not supported in this environment.",[jr]:"Instruction requires an uninitialized account",[_o]:"Instruction tries to borrow reference for an account which is already borrowed",[uo]:"Instruction left account with an outstanding borrowed reference",[co]:"Program other than the account's owner changed the size of the account data",[Wr]:"Account data too small for instruction",[lo]:"Instruction expected an executable account",[Mo]:"An account does not have enough lamports to be rent-exempt",[Po]:"Program arithmetic overflowed",[xo]:"Failed to serialize or deserialize account data",[Go]:"Builtin programs must consume compute units",[mo]:"Cross-program invocation call depth too deep",[Co]:"Computational budget exceeded",[Eo]:"Custom program error: #$code",[ro]:"Instruction contains duplicate accounts",[Ro]:"Instruction modifications of multiply-passed account differ",[No]:"Executable accounts must be rent exempt",[Ao]:"Instruction changed executable accounts data",[Oo]:"Instruction changed the balance of an executable account",[oo]:"Instruction changed executable bit of an account",[to]:"Instruction modified data of an account it does not own",[eo]:"Instruction spent from the balance of an account it does not own",[Gr]:"Generic instruction error",[Fo]:"Provided owner is not allowed",[vo]:"Account is immutable",[Do]:"Incorrect authority provided",[Yr]:"Incorrect program id for instruction",[qr]:"Insufficient funds for instruction",[Kr]:"Invalid account data for instruction",[Uo]:"Invalid account owner",[zr]:"Invalid program argument",[ho]:"Program returned invalid error code",[Hr]:"Invalid instruction data",[Io]:"Failed to reallocate account data",[To]:"Provided seeds do not result in a valid address",[$o]:"Accounts data allocations exceeded the maximum allowed per transaction",[ko]:"Max accounts exceeded",[Vo]:"Max instruction trace length exceeded",[fo]:"Length of the seed is too long for address generation",[po]:"An account required by the instruction is missing",[Xr]:"Missing required signature for instruction",[Qr]:"Instruction illegally modified the program id of an account",[so]:"Insufficient account keys for instruction",[wo]:"Cross-program invocation with unauthorized signer or writable account",[Lo]:"Failed to create program execution environment",[yo]:"Program failed to compile",[bo]:"Program failed to complete",[ao]:"Instruction modified data of a read-only account",[no]:"Instruction changed the balance of a read-only account",[go]:"Cross-program invocation reentrancy not allowed for this instruction",[io]:"Instruction modified rent epoch of an account",[Zr]:"Sum of account balances before and after instruction do not match",[Jr]:"Instruction requires an initialized account",[Vr]:"The instruction failed with the error: $errorName",[So]:"Unsupported program id",[Bo]:"Unsupported sysvar",[Nl]:"Invalid instruction plan kind: $kind.",[Ys]:"The provided instruction plan is empty.",[Js]:"No failed transaction plan result was found in the provided transaction plan result.",[js]:"This transaction plan executor does not support non-divisible sequential plans. To support them, you may create your own executor such that multi-transaction atomicity is preserved \u2014 e.g. by targetting RPCs that support transaction bundles.",[Xs]:"The provided transaction plan failed to execute. See the `transactionPlanResult` attribute for more details. Note that the `cause` property is deprecated, and a future version will not set it.",[ac]:"The configured maximum of $maxInstructions instructions per transaction is invalid. It must be a positive integer no greater than the transaction format limit of $transactionInstructionLimit instructions per transaction. Provide a `maxInstructionsPerTransaction` (on the transaction planner) or `maxInstructions` (on the message packer) value between 1 and $transactionInstructionLimit.",[nc]:"Planning this transaction message would require $numInstructions instructions, which exceeds the configured maximum of $maxInstructions instructions per transaction. This limit is configurable, and intended to leave headroom for inner instructions which are included in the maximum instruction limit for transactions. Increase `maxInstructionsPerTransaction` on the transaction planner (or `maxInstructions` on the message packer) to allow more instructions per transaction.",[Ws]:"The provided message has insufficient capacity to accommodate the next instruction(s) in this plan. Expected at least $numBytesRequired free byte(s), got $numFreeBytes byte(s).",[Sl]:"Invalid transaction plan kind: $kind.",[qs]:"No more instructions to pack; the message packer has completed the instruction plan.",[Zs]:"Unexpected instruction plan. Expected $expectedKind plan, got $actualKind plan.",[Qs]:"Unexpected transaction plan. Expected $expectedKind plan, got $actualKind plan.",[ec]:"Unexpected transaction plan result. Expected $expectedKind plan, got $actualKind plan.",[tc]:"Expected a successful transaction plan result. I.e. there is at least one failed or cancelled transaction in the plan.",[Fr]:"The instruction does not have any accounts.",[$r]:"The instruction does not have any data.",[kr]:"Expected instruction to have progress address $expectedProgramAddress, got $actualProgramAddress.",[Ta]:"Expected base58 encoded blockhash to decode to a byte array of length 32. Actual length: $actualLength.",[pa]:"The nonce `$expectedNonceValue` is no longer valid. It has advanced to `$actualNonceValue`",[hl]:"Invariant violation: Found no abortable iterable cache entry for key `$cacheKey`. It should be impossible to hit this error; please file an issue at https://sola.na/web3invariant",[Ol]:"Invariant violation: This data publisher does not publish to the channel named `$channelName`. Supported channels include $supportedChannelNames.",[El]:"Invariant violation: WebSocket message iterator state is corrupt; iterated without first resolving existing message promise. It should be impossible to hit this error; please file an issue at https://sola.na/web3invariant",[Rl]:"Invariant violation: WebSocket message iterator is missing state storage. It should be impossible to hit this error; please file an issue at https://sola.na/web3invariant",[Al]:"Invariant violation: Switch statement non-exhaustive. Received unexpected value `$unexpectedValue`. It should be impossible to hit this error; please file an issue at https://sola.na/web3invariant",[xa]:"JSON-RPC error: Internal JSON-RPC error ($__serverMessage)",[Ma]:"JSON-RPC error: Invalid method parameter(s) ($__serverMessage)",[Pa]:"JSON-RPC error: The JSON sent is not a valid `Request` object ($__serverMessage)",[Ua]:"JSON-RPC error: The method does not exist / is not available ($__serverMessage)",[Da]:"JSON-RPC error: An error occurred on the server while parsing the JSON text ($__serverMessage)",[Wa]:"$__serverMessage",[ar]:"$__serverMessage",[er]:"$__serverMessage",[Ha]:"$__serverMessage",[Va]:"Epoch rewards period still active at slot $slot",[Fa]:"$__serverMessage",[Ya]:"$__serverMessage",[Xa]:"$__serverMessage",[$a]:"Failed to query long-term storage; please try again",[Ga]:"Minimum context slot has not been reached",[Qa]:"Node is unhealthy; behind by $numSlotsBehind slots",[Ba]:"No slot history",[ja]:"No snapshot",[nr]:"Transaction simulation failed",[ka]:"Rewards cannot be found because slot $slot is not the epoch boundary. This may be due to gap in the queried node's local ledger or long-term storage",[Ja]:"$__serverMessage",[qa]:"Transaction history is not available from this node",[Za]:"$__serverMessage",[Ka]:"Transaction signature length mismatch",[tr]:"Transaction signature verification failure",[za]:"$__serverMessage",[Ur]:"The grind regex `/$source/` contains the character `$character`, which is not in the base58 alphabet and can never match a Solana address.",[yr]:"Key pair bytes must be of length 64, got $byteLength.",[vr]:"Expected private key bytes with length 32. Actual length: $actualLength.",[Dr]:"Expected base58-encoded signature to decode to a byte array of length 64. Actual length: $actualLength.",[Mr]:"The provided private key does not match the provided public key.",[xr]:"Expected base58-encoded signature string of length in the range [64, 88]. Actual length: $actualLength.",[Pr]:"Writing a key pair to disk is not supported in this environment.",[Ia]:"Lamports value must be in the range [0, 2e64-1]",[Ca]:"`$value` cannot be parsed as a `BigInt`",[ba]:"$message",[wa]:"`$value` cannot be parsed as a `Number`",[ga]:"No nonce account could be found at address `$nonceAccountAddress`",[oi]:"Expected base58 encoded application domain to decode to a byte array of length 32. Actual length: $actualLength.",[hi]:"Attempted to sign an offchain message with an address that is not a signer for it",[ri]:"Expected base58-encoded application domain string of length in the range [32, 44]. Actual length: $actualLength.",[mi]:"The content of the offchain message does not match the content that was expected. Expected content with a byte-length of $expectedBytes; got content with a byte-length of $actualBytes. The signer may have signed different data than was requested; do not trust its signature.",[Ei]:"The signer addresses in this offchain message envelope do not match the list of required signers in the message preamble. These unexpected signers were present in the envelope: `[$unexpectedSigners]`. These required signers were missing from the envelope `[$missingSigners]`.",[ni]:"The message body provided has a byte-length of $actualBytes. The maximum allowable byte-length is $maxBytes",[li]:"Expected message format $expectedMessageFormat, got $actualMessageFormat",[di]:"The message length specified in the message preamble is $specifiedLength bytes. The actual length of the message is $actualLength bytes.",[_i]:"Offchain message content must be non-empty",[si]:"Offchain message must specify the address of at least one required signer",[ui]:"Offchain message envelope must reserve space for at least one signature",[ii]:"The offchain message preamble specifies $numRequiredSignatures required signature(s), got $signaturesLength.",[pi]:"The offchain message lists different required signatories than was expected. Expected [$expectedAddresses]. Got [$actualAddresses]. The signer may have signed different data than was requested; do not trust its signature.",[Oi]:"The signatories of this offchain message must be listed in lexicographical order",[Ni]:"An address must be listed no more than once among the signatories of an offchain message",[Ri]:"Offchain message is missing signatures for addresses: $addresses.",[Si]:"Offchain message signature verification failed. Signature mismatch for required signatories [$signatoriesWithInvalidSignatures]. Missing signatures for signatories [$signatoriesWithMissingSignatures]",[ai]:"The message body provided contains characters whose codes fall outside the allowed range. In order to ensure clear-signing compatiblity with hardware wallets, the message may only contain line feeds and characters in the range [\\x20-\\x7e].",[Ai]:"Expected offchain message version $expectedVersion. Got $actualVersion.",[ci]:"This version of Kit does not support decoding offchain messages with version $unsupportedVersion. The current max supported version is 0.",[ol]:"The provided account could not be identified as an account from the $programName program.",[tl]:"The provided instruction could not be identified as an instruction from the $programName program.",[Qc]:"The provided instruction is missing some accounts. Expected at least $expectedAccountMetas account(s), got $actualAccountMetas.",[al]:"Expected resolved instruction input '$inputName' to be non-null.",[nl]:"Expected resolved instruction input '$inputName' to be of type `$expectedType`.",[rl]:"Unrecognized account type '$accountType' for the $programName program.",[el]:"Unrecognized instruction type '$instructionType' for the $programName program.",[Wc]:"The notification name must end in 'Notifications' and the API must supply a subscription plan creator function for the notification '$notificationName'.",[Yc]:"WebSocket was closed before payload could be added to the send buffer",[Xc]:"WebSocket connection closed",[jc]:"WebSocket failed to connect",[qc]:"Failed to obtain a subscription id from the server",[Kc]:"Could not find an API plan for RPC method: `$method`",[Gc]:"The $argumentLabel argument to the `$methodName` RPC method$optionalPathLabel was `$value`. This number is unsafe for use with the Solana JSON-RPC because it exceeds `Number.MAX_SAFE_INTEGER`.",[Hc]:"HTTP error ($statusCode): $message",[zc]:"HTTP header(s) forbidden: $headers. Learn more at https://developer.mozilla.org/en-US/docs/Glossary/Forbidden_header_name.",[zo]:"Multiple distinct signers were identified for address `$address`. Please ensure that you are using the same signer instance for each address.",[Ho]:"The provided value does not implement the `KeyPairSigner` interface",[Wo]:"The provided value does not implement the `MessageModifyingSigner` interface",[qo]:"The provided value does not implement the `MessagePartialSigner` interface",[Ko]:"The provided value does not implement any of the `MessageSigner` interfaces",[Xo]:"The provided value does not implement the `TransactionModifyingSigner` interface",[jo]:"The provided value does not implement the `TransactionPartialSigner` interface",[Jo]:"The provided value does not implement the `TransactionSendingSigner` interface",[Yo]:"The provided value does not implement any of the `TransactionSigner` interfaces",[Zo]:"More than one `TransactionSendingSigner` was identified.",[Qo]:"No `TransactionSendingSigner` was identified. Please provide a valid `TransactionWithSingleSendingSigner` transaction.",[ti]:"The wallet account $address cannot be used to create a transaction signer because it does not implement either the `solana:signTransaction` or `solana:signAndSendTransaction` feature. At least one of these features is required. The account supports the following features: $supportedFeatures.",[ei]:"Wallet account signers do not support signing multiple messages/transactions in a single operation",[Jc]:"This `ReactiveStreamStore` does not support retry. Use `createReactiveStoreFromDataPublisherFactory` to construct a retryable store.",[Zc]:"The stream store closed in an error state but did not report an error.",[Lr]:"Cannot export a non-extractable key.",[gr]:"No digest implementation could be found.",[pr]:"Cryptographic operations are only allowed in secure browser contexts. Read more here: https://developer.mozilla.org/en-US/docs/Web/Security/Secure_Contexts.",[fr]:`This runtime does not support the generation of Ed25519 key pairs.

Install @solana/webcrypto-ed25519-polyfill and call its \`install\` function before generating keys in environments that do not support Ed25519.

For a list of runtimes that currently support Ed25519 operations, visit https://github.com/WICG/webcrypto-secure-curves/issues/20.`,[Tr]:"No key export implementation could be found.",[Ir]:"No key generation implementation could be found.",[Cr]:"No signing implementation could be found.",[wr]:"No signature verification implementation could be found.",[La]:"Timestamp value must be in the range [-(2n ** 63n), (2n ** 63n) - 1]. `$value` given",[Is]:"Transaction processing left an account with an outstanding borrowed reference",[ds]:"Account in use",[_s]:"Account loaded twice",[us]:"Attempt to debit an account but found no record of a prior credit.",[Ds]:"Transaction loads an address table account that doesn't exist",[As]:"This transaction has already been processed",[Os]:"Blockhash not found",[Ns]:"Loader call chain is too deep",[Ts]:"Transactions are currently disabled due to cluster maintenance",[$s]:"Transaction contains a duplicate instruction ($index) that is not allowed",[Es]:"Insufficient funds for fee",[ks]:"Transaction results in an account ($accountIndex) with insufficient funds for rent",[hs]:"This account may not be used to pay transaction fees",[ms]:"Transaction contains an invalid account reference",[Ms]:"Transaction loads an address table account with invalid data",[Us]:"Transaction address table lookup uses an invalid index",[xs]:"Transaction loads an address table account with an invalid owner",[Gs]:"LoadedAccountsDataSizeLimit set for transaction must be greater than 0.",[gs]:"This program may not be used for executing instructions",[Ps]:"Transaction leaves an account with a lower balance than rent-exempt minimum",[Ls]:"Transaction loads a writable account that cannot be written",[Vs]:"Transaction exceeded max loaded accounts data size cap",[Ss]:"Transaction requires a fee but has no signature present",[Rs]:"Attempt to load a program that does not exist",[Hs]:"Execution of the program referenced by account at index $accountIndex is temporarily restricted.",[zs]:"ResanitizationNeeded",[fs]:"Transaction failed to sanitize accounts offsets correctly",[ps]:"Transaction did not pass signature verification",[vs]:"Transaction locked too many accounts",[Ks]:"Sum of account balances before and after transaction do not match",[ls]:"The transaction failed with the error `$errorName`",[ws]:"Transaction version is unsupported",[ys]:"Transaction would exceed account data limit within the block",[Fs]:"Transaction would exceed total account data limit",[bs]:"Transaction would exceed max account limit within the block",[Cs]:"Transaction would exceed max Block Cost Limit",[Bs]:"Transaction would exceed max Vote Cost Limit",[Bi]:"Attempted to sign a transaction with an address that is not a signer for it",[Di]:"Transaction is missing an address at index: $index.",[Fi]:"Transaction has no expected signers therefore it cannot be encoded",[Gi]:"Transaction size $transactionSize exceeds limit of $transactionSizeLimit bytes",[Ti]:"Transaction does not have a blockhash lifetime",[Ii]:"Transaction is not a durable nonce transaction",[wi]:"Contents of these address lookup tables unknown: $lookupTableAddresses",[Li]:"Lookup of address at index $highestRequestedIndex failed for lookup table `$lookupTableAddress`. Highest known index is $highestKnownIndex. The lookup table may have been extended since its contents were retrieved",[yi]:"No fee payer set in CompiledTransaction",[bi]:"Could not find program address at index $index",[ki]:"Failed to estimate the compute unit consumption for this transaction message. This is likely because simulating the transaction failed. Inspect the `cause` property of this error to learn more",[rs]:"Failed to estimate the loaded accounts data size for this transaction message. The RPC did not return a `loadedAccountsDataSize` value from simulation. This value is required for version 1 transactions",[Vi]:"Transaction failed when it was simulated in order to estimate the compute unit consumption. The compute unit estimate provided is for a transaction that failed when simulated and may not be representative of the compute units this transaction would consume if successful. Inspect the `cause` property of this error to learn more",[os]:"Transaction failed when it was simulated in order to estimate its resource limits. The resource limit estimates provided are for a transaction that failed when simulated and may not be representative of the resources this transaction would consume if successful. Inspect the `cause` property of this error to learn more",[xi]:"Transaction is missing a fee payer.",[Mi]:"Could not determine this transaction's signature. Make sure that the transaction has been signed by its fee payer.",[Pi]:"Transaction first instruction is not advance nonce account instruction.",[Ui]:"Transaction with no instructions cannot be durable nonce transaction.",[gi]:"This transaction includes an address (`$programAddress`) which is both invoked and set as the fee payer. Program addresses may not pay fees",[fi]:"This transaction includes an address (`$programAddress`) which is both invoked and marked writable. Program addresses may not be writable",[$i]:"The transaction message expected the transaction to have $numRequiredSignatures signatures, got $signaturesLength.",[vi]:"Transaction is missing signatures for addresses: $addresses.",[Ci]:"Transaction version must be in the range [0, 127]. `$actualVersion` given",[zi]:"This version of Kit does not support decoding transactions with version $unsupportedVersion. The current max supported version is 1.",[Hi]:"The transaction has a durable nonce lifetime (with nonce `$nonce`), but the nonce account address is in a lookup table. The lifetime constraint cannot be constructed without fetching the lookup tables for the transaction.",[ji]:"Invalid transaction config mask: $mask. Bits 0 and 1 must match (both set or both unset)",[Ki]:"Transaction message bytes are malformed: $messageBytes",[Wi]:"Transaction message bytes are empty, so the transaction cannot be encoded",[qi]:"Transaction bytes are empty, so no transaction can be decoded",[Yi]:"Transaction version 0 must be encoded with signatures first. This transaction was encoded with first byte $firstByte, which is expected to be a signature count for v0 transactions.",[Xi]:"The provided transaction bytes expect that there should be $numExpectedSignatures signatures, but the bytes are not long enough to contain a transaction message with this many signatures. The provided bytes are $transactionBytesLength bytes long.",[Ji]:"The transaction has a durable nonce lifetime, but the nonce account index is invalid. Expected a nonce account index less than $numberOfStaticAccounts, got $nonceAccountIndex.",[Zi]:"The transaction config value for $configName has the incorrect kind. Expected $expectedKind, got $actualKind.",[Qi]:"The transaction does not have the same number of instruction headers and instruction payloads. Got $numInstructionHeaders instruction headers, and $numInstructionPayloads instruction payloads.",[es]:"Transaction has $actualCount unique signer addresses but the maximum allowed is $maxAllowed",[ts]:"Transaction has $actualCount unique account addresses but the maximum allowed is $maxAllowed",[ns]:"Transaction has $actualCount instructions but the maximum allowed is $maxAllowed",[as]:"The instruction at index $instructionIndex has $actualCount account references but the maximum allowed is $maxAllowed",[is]:"Could not find an account address at index $index while decompiling an instruction",[ss]:"`getTransaction` responses fetched with `encoding: 'jsonParsed'` cannot be decoded. Re-fetch the transaction with `encoding: 'base64'`, `'base58'`, or `'json'`",[cs]:"Could not recognize the shape of this `getTransaction` response. Expected a response fetched with `encoding: 'base64'`, `'base58'`, or `'json'`",[_l]:"`$hookName` requires the following capabilities to be installed on the client: [$capabilities]. $providerHint",[dl]:"`$hookName` was called outside of a `ClientProvider`. Mount a `<ClientProvider client={client}>` in the ancestor tree.",[ul]:"The subscription closed in an error state but did not report an error.",[il]:"Cannot $operation: no wallet connected",[sl]:"No signing wallet connected (status: $status)",[cl]:"Connected wallet does not support signing",[ll]:'Account $address is not available in wallet "$walletName"'};function gl(t,e={}){{let n=`Solana error #${t}; Decode this error by running \`npx @solana/errors decode -- ${t}`;return Object.keys(e).length&&(n+=` '${pl(e)}'`),`${n}\``}}var me=class extends Error{cause=this.cause;context;constructor(...[t,e]){let n,a;e&&Object.entries(Object.getOwnPropertyDescriptors(e)).forEach(([o,i])=>{o==="cause"?a={cause:i.value}:(n===void 0&&(n={__code:t}),Object.defineProperty(n,o,i))});let r=gl(t,n);super(r,a),this.context=Object.freeze(n===void 0?{__code:t}:n),this.name="SolanaError"}};function fl(t,e){return"fixedSize"in e?e.fixedSize:e.getSizeFromValue(t)}function pe(t){return Object.freeze({...t,encode:e=>{let n=new Uint8Array(fl(e,t));return t.write(e,n,0),n}})}function ge(t){return Object.freeze({...t,decode:(e,n=0)=>t.read(e,n)[0]})}function Tl(t,e,n=e){if(!e.match(new RegExp(`^[${t}]*$`)))throw new me(oe,{alphabet:t,base:t.length,value:n})}var Il=t=>pe({getSizeFromValue:e=>{let[n,a]=wt(e,t[0]);if(!a)return e.length;let r=Lt(a,t);return n.length+Math.ceil(r.toString(16).length/2)},write(e,n,a){if(Tl(t,e),e==="")return a;let[r,o]=wt(e,t[0]);if(!o)return n.set(new Uint8Array(r.length).fill(0),a),a+r.length;let i=Lt(o,t),s=[];for(;i>0n;)s.unshift(Number(i%256n)),i/=256n;let c=[...Array(r.length).fill(0),...s];return n.set(c,a),a+c.length}}),Cl=t=>ge({read(e,n){let a=n===0||n<=-e.byteLength?e:e.slice(n);if(a.length===0)return["",e.length];let r=a.findIndex(c=>c!==0);r=r===-1?a.length:r;let o=t[0].repeat(r);if(r===a.length)return[o,e.length];let i=a.slice(r).reduce((c,l)=>c*256n+BigInt(l),0n),s=wl(i,t);return[o+s,e.length]}});function wt(t,e){let[n,a]=t.split(new RegExp(`((?!${e}).*)`));return[n,a]}function Lt(t,e){let n=BigInt(e.length),a=0n;for(let r of t)a*=n,a+=BigInt(e.indexOf(r));return a}function wl(t,e){let n=BigInt(e.length),a=[];for(;t>0n;)a.unshift(e[Number(t%n)]),t/=n;return a.join("")}var vt="123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz",Dt=()=>Il(vt),xt=()=>Cl(vt);var bt="ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/",Mt=()=>pe({getSizeFromValue:t=>{try{return atob(t).length}catch{throw new me(oe,{alphabet:bt,base:64,value:t})}},write(t,e,n){try{let a=atob(t).split("").map(r=>r.charCodeAt(0));return e.set(a,n),a.length+n}catch{throw new me(oe,{alphabet:bt,base:64,value:t})}}}),ze=()=>ge({read(t,e=0){let n=t.slice(e);return[btoa(String.fromCharCode(...n)),t.length]}});var Ll=t=>t.replace(/\u0000/g,"");var bl=globalThis.TextDecoder,yt=globalThis.TextEncoder,He=()=>{let t;return pe({getSizeFromValue:e=>(t||=new yt).encode(e).length,write:(e,n,a)=>{let r=(t||=new yt).encode(e);return n.set(r,a),a+r.length}})},Ut=()=>{let t;return ge({read(e,n){let a=(t||=new bl).decode(e.slice(n));return[Ll(a),e.length]}})};function Pt(t){return ze().decode(He().encode(t))}function F(t,e){let n=ze().decode(t);return e?n.replace(/\+/g,"-").replace(/\//g,"_").replace(/=+$/,""):n}function ie(t){return Mt().encode(t)}function Bt(t){return xt().decode(t)}function yl(t){return Dt().encode(t)}function Ft(t){return Bt(ie(t))}function fe(t){return F(new Uint8Array(t))}function X(t){return Bt(t)}function Ke(t){return yl(t)}function P(t){return F(t)}function I(t){return ie(t)}function We(t){return Ut().decode(t)}function qe(t){return He().encode(t)}var Ye="solana:mainnet";var vl=function(t,e,n,a){if(n==="a"&&!a)throw new TypeError("Private accessor was defined without a getter");if(typeof e=="function"?t!==e||!a:!e.has(t))throw new TypeError("Cannot read private member from an object whose class did not declare it");return n==="m"?a:n==="a"?a.call(t):a?a.value:e.get(t)},Dl=function(t,e,n,a,r){if(a==="m")throw new TypeError("Private method is not writable");if(a==="a"&&!r)throw new TypeError("Private accessor was defined without a setter");if(typeof e=="function"?t!==e||!r:!e.has(t))throw new TypeError("Cannot write private member to an object whose class did not declare it");return a==="a"?r.call(t,n):r?r.value=n:e.set(t,n),n},Te;function je(t){let e=({register:n})=>n(t);try{window.dispatchEvent(new Xe(e))}catch(n){console.error(`wallet-standard:register-wallet event could not be dispatched
`,n)}try{window.addEventListener("wallet-standard:app-ready",({detail:n})=>e(n))}catch(n){console.error(`wallet-standard:app-ready event listener could not be added
`,n)}}var Xe=class extends Event{get detail(){return vl(this,Te,"f")}get type(){return"wallet-standard:register-wallet"}constructor(e){super("wallet-standard:register-wallet",{bubbles:!1,cancelable:!1,composed:!1}),Te.set(this,void 0),Dl(this,Te,e,"f")}preventDefault(){throw new Error("preventDefault cannot be called")}stopImmediatePropagation(){throw new Error("stopImmediatePropagation cannot be called")}stopPropagation(){throw new Error("stopPropagation cannot be called")}};Te=new WeakMap;var xl="(?<domain>[^\\n]+?) wants you to sign in with your Solana account:\\n",Ml="(?<address>[^\\n]+)(?:\\n|$)",Ul="(?:\\n(?<statement>[\\S\\s]*?)(?:\\n|$))??",Pl="(?:\\nURI: (?<uri>[^\\n]+))?",Bl="(?:\\nVersion: (?<version>[^\\n]+))?",Fl="(?:\\nChain ID: (?<chainId>[^\\n]+))?",$l="(?:\\nNonce: (?<nonce>[^\\n]+))?",kl="(?:\\nIssued At: (?<issuedAt>[^\\n]+))?",Vl="(?:\\nExpiration Time: (?<expirationTime>[^\\n]+))?",Gl="(?:\\nNot Before: (?<notBefore>[^\\n]+))?",zl="(?:\\nRequest ID: (?<requestId>[^\\n]+))?",Hl="(?:\\nResources:(?<resources>(?:\\n- [^\\n]+)*))?",Kl=`${Pl}${Bl}${Fl}${$l}${kl}${Vl}${Gl}${zl}${Hl}`,nu=new RegExp(`^${xl}${Ml}${Ul}${Kl}\\n*$`);function $t(t){let e=`${t.domain} wants you to sign in with your Solana account:
`;e+=`${t.address}`,t.statement&&(e+=`

${t.statement}`);let n=[];if(t.uri&&n.push(`URI: ${t.uri}`),t.version&&n.push(`Version: ${t.version}`),t.chainId&&n.push(`Chain ID: ${t.chainId}`),t.nonce&&n.push(`Nonce: ${t.nonce}`),t.issuedAt&&n.push(`Issued At: ${t.issuedAt}`),t.expirationTime&&n.push(`Expiration Time: ${t.expirationTime}`),t.notBefore&&n.push(`Not Before: ${t.notBefore}`),t.requestId&&n.push(`Request ID: ${t.requestId}`),t.resources){n.push("Resources:");for(let a of t.resources)n.push(`- ${a}`)}return n.length&&(e+=`

${n.join(`
`)}`),e}var S={ERROR_ASSOCIATION_PORT_OUT_OF_RANGE:"ERROR_ASSOCIATION_PORT_OUT_OF_RANGE",ERROR_REFLECTOR_ID_OUT_OF_RANGE:"ERROR_REFLECTOR_ID_OUT_OF_RANGE",ERROR_FORBIDDEN_WALLET_BASE_URL:"ERROR_FORBIDDEN_WALLET_BASE_URL",ERROR_SECURE_CONTEXT_REQUIRED:"ERROR_SECURE_CONTEXT_REQUIRED",ERROR_SESSION_CLOSED:"ERROR_SESSION_CLOSED",ERROR_SESSION_TIMEOUT:"ERROR_SESSION_TIMEOUT",ERROR_WALLET_NOT_FOUND:"ERROR_WALLET_NOT_FOUND",ERROR_INVALID_PROTOCOL_VERSION:"ERROR_INVALID_PROTOCOL_VERSION",ERROR_BROWSER_NOT_SUPPORTED:"ERROR_BROWSER_NOT_SUPPORTED",ERROR_LOOPBACK_ACCESS_BLOCKED:"ERROR_LOOPBACK_ACCESS_BLOCKED",ERROR_ASSOCIATION_CANCELLED:"ERROR_ASSOCIATION_CANCELLED",ERROR_ILLEGAL_TRANSPORT_STATE:"ERROR_ILLEGAL_TRANSPORT_STATE"},O=class extends Error{data;code;constructor(...t){let[e,n,a]=t;super(n),this.code=e,this.data=a,this.name="SolanaMobileWalletAdapterError"}};var Je=class extends Error{data;code;jsonRpcMessageId;constructor(...t){let[e,n,a,r]=t;super(a),this.code=n,this.data=r,this.jsonRpcMessageId=e,this.name="SolanaMobileWalletAdapterProtocolError"}};async function Ie(t,e){let n=await crypto.subtle.exportKey("raw",t),a=await crypto.subtle.sign({hash:"SHA-256",name:"ECDSA"},e,n),r=new Uint8Array(n.byteLength+a.byteLength);return r.set(new Uint8Array(n),0),r.set(new Uint8Array(a),n.byteLength),r}function Wl(t){return $t(t)}function ql(t){return Pt(Wl(t)).replace(/\+/g,"-").replace(/\//g,"_").replace(/=+$/,"")}var Yl="solana:signTransactions",kt="solana:cloneAuthorization";function zt(t,e){return new Proxy({},{get(n,a){return a==="then"?null:(n[a]==null&&(n[a]=async function(r){let{method:o,params:i}=Xl(a,r,t),s=await e(o,i);return o==="authorize"&&i.sign_in_payload&&!s.sign_in_result&&(s.sign_in_result=await Jl(i.sign_in_payload,s,e)),jl(a,s,t)}),n[a])},defineProperty(){return!1},deleteProperty(){return!1}})}function Xl(t,e,n){let a=e,r=t.toString().replace(/[A-Z]/g,o=>`_${o.toLowerCase()}`).toLowerCase();switch(t){case"authorize":{let o=a,{chain:i}=o;if(n==="legacy"){switch(i){case"solana:testnet":i="testnet";break;case"solana:devnet":i="devnet";break;case"solana:mainnet":i="mainnet-beta";break;default:i=o.cluster}o.cluster=i,a=o}else{switch(i){case"testnet":case"devnet":i=`solana:${i}`;break;case"mainnet-beta":i="solana:mainnet"}o.chain=i,a=o}}case"reauthorize":{let{auth_token:o,identity:i}=a;o&&(n==="legacy"?(r="reauthorize",a={auth_token:o,identity:i}):r="authorize");break}}return{method:r,params:a}}function jl(t,e,n){switch(t){case"getCapabilities":{let a=e;switch(n){case"legacy":{let r=[Yl];return a.supports_clone_authorization===!0&&r.push(kt),{...a,features:r}}case"v1":return{...a,supports_sign_and_send_transactions:!0,supports_clone_authorization:a.features.includes(kt)}}}}return e}async function Jl(t,e,n){let a=t.domain??window.location.host,r=e.accounts[0].address,o=ql({...t,domain:a,address:Ft(r)}),i=await n("sign_messages",{addresses:[r],payloads:[o]}),s=ie(i.signed_payloads[0]),c=F(s.slice(0,s.length-64)),l=F(s.slice(s.length-64));return{address:r,signed_message:c.length==0?o:c,signature:l}}function Zl(t){if(t>=4294967296)throw new Error("Outbound sequence number overflow. The maximum sequence number is 32-bytes.");let e=new ArrayBuffer(4);return new DataView(e).setUint32(0,t,!1),new Uint8Array(e)}var Ql=12;async function ed(t,e,n){let a=Zl(e),r=new Uint8Array(Ql);crypto.getRandomValues(r);let o=await crypto.subtle.encrypt(Kt(a,r),n,qe(t)),i=new Uint8Array(a.byteLength+r.byteLength+o.byteLength);return i.set(new Uint8Array(a),0),i.set(new Uint8Array(r),a.byteLength),i.set(new Uint8Array(o),a.byteLength+r.byteLength),i}async function Ht(t,e){let n=t.slice(0,4),a=t.slice(4,16),r=t.slice(16),o=await crypto.subtle.decrypt(Kt(n,a),e,r);return We(new Uint8Array(o))}function Kt(t,e){return{additionalData:t,iv:e,name:"AES-GCM",tagLength:128}}async function Wt(){return await crypto.subtle.generateKey({name:"ECDSA",namedCurve:"P-256"},!1,["sign"])}async function Ce(){return await crypto.subtle.generateKey({name:"ECDH",namedCurve:"P-256"},!1,["deriveKey","deriveBits"])}function td(){return qt(49152+Math.floor(Math.random()*16384))}function qt(t){if(t<49152||t>65535)throw new O(S.ERROR_ASSOCIATION_PORT_OUT_OF_RANGE,`Association port number must be between 49152 and 65535. ${t} given.`,{port:t});return t}function Yt(t){return t.replace(/[/+=]/g,e=>({"/":"_","+":"-","=":"."})[e])}var nd="solana-wallet";function Vt(t){return t.replace(/(^\/+|\/+$)/g,"").split("/")}function Xt(t,e){let n=null;if(e){try{n=new URL(e)}catch{}if(n?.protocol!=="https:")throw new O(S.ERROR_FORBIDDEN_WALLET_BASE_URL,"Base URLs supplied by wallets must be valid `https` URLs")}n||=new URL(`${nd}:/`);let a=t.startsWith("/")?t:[...Vt(n.pathname),...Vt(t)].join("/");return new URL(a,n)}async function ad(t,e,n,a=["v1"]){let r=qt(e),o=await crypto.subtle.exportKey("raw",t),i=fe(o),s=Xt("v1/associate/local",n);return s.searchParams.set("association",Yt(i)),s.searchParams.set("port",`${r}`),a.forEach(c=>{s.searchParams.set("v",c)}),s}async function rd(t,e,n,a,r=["v1"]){let o=await crypto.subtle.exportKey("raw",t),i=fe(o),s=Xt("v1/associate/remote",a);return s.searchParams.set("association",Yt(i)),s.searchParams.set("reflector",`${e}`),s.searchParams.set("id",`${F(n,!0)}`),r.forEach(c=>{s.searchParams.set("v",c)}),s}async function jt(t,e){let n=JSON.stringify(t),a=t.id;return ed(n,a,e)}async function Jt(t,e){let n=await Ht(t,e),a=JSON.parse(n);if(Object.hasOwnProperty.call(a,"error"))throw new Je(a.id,a.error.code,a.error.message);return a}async function Zt(t,e,n){let[a,r]=await Promise.all([crypto.subtle.exportKey("raw",e),crypto.subtle.importKey("raw",t.slice(0,65),{name:"ECDH",namedCurve:"P-256"},!1,[])]),o=await crypto.subtle.deriveBits({name:"ECDH",public:r},n,256),i=await crypto.subtle.importKey("raw",o,"HKDF",!1,["deriveKey"]);return await crypto.subtle.deriveKey({name:"HKDF",hash:"SHA-256",salt:new Uint8Array(a),info:new Uint8Array},i,{name:"AES-GCM",length:128},!1,["encrypt","decrypt"])}async function Qt(t,e){let n=await Ht(t,e),a=JSON.parse(n),r="legacy";if(Object.hasOwnProperty.call(a,"v"))switch(a.v){case 1:case"1":case"v1":r="v1";break;case"legacy":r="legacy";break;default:throw new O(S.ERROR_INVALID_PROTOCOL_VERSION,`Unknown/unsupported protocol version: ${a.v}`)}return{protocol_version:r}}var we={Firefox:0,Other:1};function od(){return navigator.userAgent.indexOf("Firefox/")!==-1?we.Firefox:we.Other}function id(){return new Promise((t,e)=>{function n(){clearTimeout(r),window.removeEventListener("blur",a)}function a(){n(),t()}window.addEventListener("blur",a);let r=setTimeout(()=>{n(),e()},3e3)})}var j=null;function sd(t){(j==null||!j.isConnected)&&(j=document.createElement("iframe"),j.style.display="none",document.body.appendChild(j)),j.contentWindow.location.href=t.toString()}async function cd(t){if(t.protocol==="https:")window.location.assign(t);else try{switch(od()){case we.Firefox:sd(t);break;case we.Other:{let e=id();window.location.assign(t),await e;break}}}catch{throw new O(S.ERROR_WALLET_NOT_FOUND,"Found no installed wallet that supports the mobile wallet protocol.")}}async function ld(t,e){let n=td();return await cd(await ad(t,n,e)),n}var Le={retryDelayScheduleMs:[150,150,200,500,500,750,750,1e3],timeoutMs:3e4},en="com.solana.mobilewalletadapter.v1",Gt="com.solana.mobilewalletadapter.v1.base64";function tn(){if(typeof window>"u"||window.isSecureContext!==!0)throw new O(S.ERROR_SECURE_CONTEXT_REQUIRED,"The mobile wallet adapter protocol must be used in a secure context (`https`).")}function nn(t){let e;try{e=new URL(t)}catch{throw new O(S.ERROR_FORBIDDEN_WALLET_BASE_URL,"Invalid base URL supplied by wallet")}if(e.protocol!=="https:")throw new O(S.ERROR_FORBIDDEN_WALLET_BASE_URL,"Base URLs supplied by wallets must be valid `https` URLs")}function be(t){return new DataView(t).getUint32(0,!1)}function dd(t){let e=new Uint8Array(t),n=t.byteLength,a=10,r=0,o=0,i;do{if(o>=n||o>a)throw new RangeError("Failed to decode varint");i=e[o++],r|=(i&127)<<7*o}while(i>=128);return{value:r,offset:o}}function _d(t){let{value:e,offset:n}=dd(t);return new Uint8Array(t.slice(n,n+e))}async function an(t){tn();let e=await Wt(),n=`ws://localhost:${await ld(e.publicKey,t?.baseUri)}/solana-wallet`,a,r=(()=>{let N=[...Le.retryDelayScheduleMs];return()=>N.length>1?N.shift():N[0]})(),o=1,i=0,s={__type:"disconnected"},c,l=!1,d;return{close:()=>{c.close(),d()},wallet:new Promise((N,E)=>{let T={},b=async()=>{if(s.__type!=="connecting"){console.warn(`Expected adapter state to be \`connecting\` at the moment the websocket opens. Got \`${s.__type}\`.`);return}c.removeEventListener("open",b);let{associationKeypair:_}=s,u=await Ce();c.send(await Ie(u.publicKey,_.privateKey)),s={__type:"hello_req_sent",associationPublicKey:_.publicKey,ecdhPrivateKey:u.privateKey}},p=_=>{_.wasClean?s={__type:"disconnected"}:E(new O(S.ERROR_SESSION_CLOSED,`The wallet session dropped unexpectedly (${_.code}: ${_.reason}).`,{closeEvent:_})),v()},L=async _=>{v(),Date.now()-a>=Le.timeoutMs?E(new O(S.ERROR_SESSION_TIMEOUT,`Failed to connect to the wallet websocket at ${n}.`)):(await new Promise(u=>{let R=r();U=window.setTimeout(u,R)}),y())},C=async _=>{let u=await _.data.arrayBuffer();switch(s.__type){case"connecting":{if(u.byteLength!==0){E(new O(S.ERROR_ILLEGAL_TRANSPORT_STATE,"Encountered unexpected message while connecting"));return}let R=await Ce();c.send(await Ie(R.publicKey,e.privateKey)),s={__type:"hello_req_sent",associationPublicKey:e.publicKey,ecdhPrivateKey:R.privateKey};break}case"connected":try{let R=be(u.slice(0,4));if(R!==i+1)throw new O(S.ERROR_ILLEGAL_TRANSPORT_STATE,"Encrypted message has invalid sequence number");i=R;let g=await Jt(u,s.sharedSecret),w=T[g.id];delete T[g.id],w.resolve(g.result)}catch(R){if(R instanceof Je){let g=T[R.jsonRpcMessageId];delete T[R.jsonRpcMessageId],g.reject(R)}else throw R}break;case"hello_req_sent":{if(u.byteLength===0){let x=await Ce();c.send(await Ie(x.publicKey,e.privateKey)),s={__type:"hello_req_sent",associationPublicKey:e.publicKey,ecdhPrivateKey:x.privateKey};break}let R=await Zt(u,s.associationPublicKey,s.ecdhPrivateKey),g=u.slice(65),w=g.byteLength!==0?await(async()=>{let x=be(g.slice(0,4));return x!==i+1?(E(new O(S.ERROR_ILLEGAL_TRANSPORT_STATE,"Encrypted message has invalid sequence number")),c.close(),{protocol_version:"v1"}):(i=x,Qt(g,R))})():{protocol_version:"legacy"};s={__type:"connected",sharedSecret:R,sessionProperties:w};let ne=zt(w.protocol_version,async(x,Ae)=>{let Oe=o++;return c.send(await jt({id:Oe,jsonrpc:"2.0",method:x,params:Ae??{}},R)),new Promise((Ne,ae)=>{T[Oe]={resolve(re){switch(x){case"authorize":case"reauthorize":{let{wallet_uri_base:Se}=re;if(Se!=null)try{nn(Se)}catch(ua){ae(ua);return}break}}Ne(re)},reject:ae}})});l=!0;try{N(ne)}catch(x){E(x)}break}}};d=()=>{c.removeEventListener("message",C),v(),l||E(new O(S.ERROR_SESSION_CLOSED,"The wallet session was closed before connection.",{closeEvent:new CloseEvent("socket was closed before connection")}))};let v,U,y=()=>{v&&v(),s={__type:"connecting",associationKeypair:e},a===void 0&&(a=Date.now()),c=new WebSocket(n,[en]),c.addEventListener("open",b),c.addEventListener("close",p),c.addEventListener("error",L),c.addEventListener("message",C),v=()=>{window.clearTimeout(U),c.removeEventListener("open",b),c.removeEventListener("close",p),c.removeEventListener("error",L),c.removeEventListener("message",C)}};y()})}}async function rn(t){tn();let e=await Wt(),n=`wss://${t?.remoteHostAuthority}/reflect`,a,r=(()=>{let p=[...Le.retryDelayScheduleMs];return()=>p.length>1?p.shift():p[0]})(),o=1,i=0,s,c={__type:"disconnected"},l,d,N=async p=>{if(s=="base64"){let L=await p.data;return ie(L).buffer}else return await p.data.arrayBuffer()},E=await new Promise((p,L)=>{let C=async()=>{if(c.__type!=="connecting"){console.warn(`Expected adapter state to be \`connecting\` at the moment the websocket opens. Got \`${c.__type}\`.`);return}l.protocol.includes(Gt)?s="base64":s="binary",l.removeEventListener("open",C)},v=R=>{R.wasClean?c={__type:"disconnected"}:L(new O(S.ERROR_SESSION_CLOSED,`The wallet session dropped unexpectedly (${R.code}: ${R.reason}).`,{closeEvent:R})),d()},U=async R=>{d(),Date.now()-a>=Le.timeoutMs?L(new O(S.ERROR_SESSION_TIMEOUT,`Failed to connect to the wallet websocket at ${n}.`)):(await new Promise(g=>{let w=r();_=window.setTimeout(g,w)}),u())},y=async R=>{let g=await N(R);if(c.__type==="connecting"){if(g.byteLength==0){L(new O(S.ERROR_ILLEGAL_TRANSPORT_STATE,"Encountered unexpected message while connecting")),l.close();return}let w=_d(g);c={__type:"reflector_id_received",reflectorId:w};let ne=await rd(e.publicKey,t.remoteHostAuthority,w,t?.baseUri);l.removeEventListener("message",y),p(ne)}},_,u=()=>{d&&d(),c={__type:"connecting",associationKeypair:e},a===void 0&&(a=Date.now()),l=new WebSocket(n,[en,Gt]),l.addEventListener("open",C),l.addEventListener("close",v),l.addEventListener("error",U),l.addEventListener("message",y),d=()=>{window.clearTimeout(_),l.removeEventListener("open",C),l.removeEventListener("close",v),l.removeEventListener("error",U),l.removeEventListener("message",y)}};u()}),T=!1,b;return{associationUrl:E,close:()=>{l.close(),b()},wallet:new Promise((p,L)=>{let C={},v=async U=>{let y=await N(U);switch(c.__type){case"reflector_id_received":{if(y.byteLength!==0){L(new O(S.ERROR_ILLEGAL_TRANSPORT_STATE,"Encountered unexpected message while awaiting reflection")),l.close();return}let _=await Ce(),u=await Ie(_.publicKey,e.privateKey);s=="base64"?l.send(F(u)):l.send(u),c={__type:"hello_req_sent",associationPublicKey:e.publicKey,ecdhPrivateKey:_.privateKey};break}case"connected":try{let _=be(y.slice(0,4));if(_!==i+1)throw new O(S.ERROR_ILLEGAL_TRANSPORT_STATE,"Encrypted message has invalid sequence number");i=_;let u=await Jt(y,c.sharedSecret),R=C[u.id];delete C[u.id],R.resolve(u.result)}catch(_){if(_ instanceof Je){let u=C[_.jsonRpcMessageId];delete C[_.jsonRpcMessageId],u.reject(_)}else throw _}break;case"hello_req_sent":{let _=await Zt(y,c.associationPublicKey,c.ecdhPrivateKey),u=y.slice(65),R=u.byteLength!==0?await(async()=>{let w=be(u.slice(0,4));return w!==i+1?(L(new O(S.ERROR_ILLEGAL_TRANSPORT_STATE,"Encrypted message has invalid sequence number")),l.close(),{protocol_version:"v1"}):(i=w,Qt(u,_))})():{protocol_version:"legacy"};c={__type:"connected",sharedSecret:_,sessionProperties:R};let g=zt(R.protocol_version,async(w,ne)=>{let x=o++,Ae=await jt({id:x,jsonrpc:"2.0",method:w,params:ne??{}},_);return s=="base64"?l.send(F(Ae)):l.send(Ae),new Promise((Oe,Ne)=>{C[x]={resolve(ae){switch(w){case"authorize":case"reauthorize":{let{wallet_uri_base:re}=ae;if(re!=null)try{nn(re)}catch(Se){Ne(Se);return}break}}Oe(ae)},reject:Ne}})});T=!0;try{p(g)}catch(w){L(w)}break}}};l.addEventListener("message",v),b=()=>{l.removeEventListener("message",v),d(),T||L(new O(S.ERROR_SESSION_CLOSED,"The wallet session was closed before connection.",{closeEvent:new CloseEvent("socket was closed before connection")}))}})}}var $="solana:signAndSendTransaction";var ye="solana:signIn";var ve="solana:signMessage";var k="solana:signTransaction";var Ze="standard:connect";var Qe="standard:disconnect";var et="standard:events";var ia=Sa(oa(),1),Tt="SolanaMobileWalletAdapterDefaultAuthorizationCache";function ju(){let t;try{t=window.localStorage}catch{}return{async clear(){if(t)try{t.removeItem(Tt)}catch{}},async get(){if(t)try{let e=JSON.parse(t.getItem(Tt));if(e&&e.accounts){let n=e.accounts.map(a=>({...a,publicKey:"publicKey"in a?new Uint8Array(Object.values(a.publicKey)):Ke(a.address)}));return{...e,accounts:n}}else return e||void 0}catch{}},async set(e){if(t)try{t.setItem(Tt,JSON.stringify(e))}catch{}}}}function Ju(){return{async select(t){return t.length===1?t[0]:t.includes(Ye)?Ye:t[0]}}}var o_=`
<div class="mobile-wallet-adapter-embedded-modal-container" role="dialog" aria-modal="true" aria-labelledby="modal-title">
    <div data-modal-close style="position: absolute; width: 100%; height: 100%;"></div>
	<div class="mobile-wallet-adapter-embedded-modal-card">
		<div>
			<button data-modal-close class="mobile-wallet-adapter-embedded-modal-close">
				<svg width="14" height="14">
					<path d="M 6.7125,8.3036995 1.9082,13.108199 c -0.2113,0.2112 -0.4765,0.3168 -0.7957,0.3168 -0.3192,0 -0.5844,-0.1056 -0.7958,-0.3168 C 0.1056,12.896899 0,12.631699 0,12.312499 c 0,-0.3192 0.1056,-0.5844 0.3167,-0.7958 L 5.1212,6.7124995 0.3167,1.9082 C 0.1056,1.6969 0,1.4317 0,1.1125 0,0.7933 0.1056,0.5281 0.3167,0.3167 0.5281,0.1056 0.7933,0 1.1125,0 1.4317,0 1.6969,0.1056 1.9082,0.3167 L 6.7125,5.1212 11.5167,0.3167 C 11.7281,0.1056 11.9933,0 12.3125,0 c 0.3192,0 0.5844,0.1056 0.7957,0.3167 0.2112,0.2114 0.3168,0.4766 0.3168,0.7958 0,0.3192 -0.1056,0.5844 -0.3168,0.7957 L 8.3037001,6.7124995 13.1082,11.516699 c 0.2112,0.2114 0.3168,0.4766 0.3168,0.7958 0,0.3192 -0.1056,0.5844 -0.3168,0.7957 -0.2113,0.2112 -0.4765,0.3168 -0.7957,0.3168 -0.3192,0 -0.5844,-0.1056 -0.7958,-0.3168 z" />
				</svg>
			</button>
		</div>
		<div class="mobile-wallet-adapter-embedded-modal-content"></div>
	</div>
</div>
`,i_=`
.mobile-wallet-adapter-embedded-modal-container {
    display: flex; /* Use flexbox to center content */
    justify-content: center; /* Center horizontally */
    align-items: center; /* Center vertically */
    position: fixed; /* Stay in place */
    z-index: 2147483647; /* Sit on top */
    left: 0;
    top: 0;
    width: 100%; /* Full width */
    height: 100%; /* Full height */
    background-color: rgba(0,0,0,0.4); /* Black w/ opacity */
    overflow-y: auto; /* enable scrolling */
}

.mobile-wallet-adapter-embedded-modal-card {
    display: flex;
    flex-direction: column;
    margin: auto 20px;
    max-width: 780px;
    padding: 20px;
    border-radius: 24px;
    background: #ffffff;
    font-family: "Inter Tight", "PT Sans", Calibri, sans-serif;
    transform: translateY(-200%);
    animation: slide-in 0.5s forwards;
}

@keyframes slide-in {
    100% { transform: translateY(0%); }
}

.mobile-wallet-adapter-embedded-modal-close {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    cursor: pointer;
    background: #e4e9e9;
    border: none;
    border-radius: 50%;
}

.mobile-wallet-adapter-embedded-modal-close:focus-visible {
    outline-color: red;
}

.mobile-wallet-adapter-embedded-modal-close svg {
    fill: #546266;
    transition: fill 200ms ease 0s;
}

.mobile-wallet-adapter-embedded-modal-close:hover svg {
    fill: #fff;
}
`,s_=`
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Inter+Tight:ital,wght@0,100..900;1,100..900&display=swap" rel="stylesheet">
`,he=class{#e=null;#n={};#r=!1;dom=null;constructor(){this.init=this.init.bind(this),this.#e=document.getElementById("mobile-wallet-adapter-embedded-root-ui")}async init(){console.log("Injecting modal"),this.#d()}open=()=>{console.debug("Modal open"),this.#_(),this.#e&&(this.#e.style.display="flex")};close=(t=void 0)=>{console.debug("Modal close"),this.#s(),this.#e&&(this.#e.style.display="none"),this.#n.close?.forEach(e=>e(t))};addEventListener(t,e){return this.#n[t]?.push(e)||(this.#n[t]=[e]),()=>this.removeEventListener(t,e)}removeEventListener(t,e){this.#n[t]=this.#n[t]?.filter(n=>e!==n)}#d(){if(document.getElementById("mobile-wallet-adapter-embedded-root-ui")){this.#e||(this.#e=document.getElementById("mobile-wallet-adapter-embedded-root-ui"));return}this.#e=document.createElement("div"),this.#e.id="mobile-wallet-adapter-embedded-root-ui",this.#e.innerHTML=o_,this.#e.style.display="none";let t=this.#e.querySelector(".mobile-wallet-adapter-embedded-modal-content");t&&(t.innerHTML=this.contentHtml);let e=document.createElement("style");e.id="mobile-wallet-adapter-embedded-modal-styles",e.textContent=i_+this.contentStyles;let n=document.createElement("div");n.innerHTML=s_,this.dom=n.attachShadow({mode:"closed"}),this.dom.appendChild(e),this.dom.appendChild(this.#e),document.body.appendChild(n)}#_(){!this.#e||this.#r||([...this.#e.querySelectorAll("[data-modal-close]")].forEach(t=>t?.addEventListener("click",this.close)),window.addEventListener("load",this.close),document.addEventListener("keydown",this.#t),this.#r=!0)}#s(){this.#r&&(window.removeEventListener("load",this.close),document.removeEventListener("keydown",this.#t),this.#e&&([...this.#e.querySelectorAll("[data-modal-close]")].forEach(t=>t?.removeEventListener("click",this.close)),this.#r=!1))}#t=t=>{t.key==="Escape"&&this.close(t)}},c_="To use mobile wallet adapter, you must have a compatible mobile wallet application installed on your device.",l_="This browser appears to be incompatible with mobile wallet adapter. Open this page in a compatible mobile browser app and try again.",d_=class extends he{contentStyles=u_;contentHtml=__;initWithError(t){super.init(),this.populateError(t)}populateError(t){let e=this.dom?.getElementById("mobile-wallet-adapter-error-message"),n=this.dom?.getElementById("mobile-wallet-adapter-error-action");if(e){if(t.name==="SolanaMobileWalletAdapterError")switch(t.code){case"ERROR_WALLET_NOT_FOUND":e.innerHTML=c_,n&&n.addEventListener("click",()=>{window.location.href="https://solanamobile.com/wallets"});return;case"ERROR_BROWSER_NOT_SUPPORTED":e.innerHTML=l_,n&&(n.style.display="none");return}e.innerHTML=`An unexpected error occurred: ${t.message}`}else console.log("Failed to locate error dialog element")}},__=`
<svg class="mobile-wallet-adapter-embedded-modal-error-icon" xmlns="http://www.w3.org/2000/svg" height="50px" viewBox="0 -960 960 960" width="50px" fill="#000000"><path d="M 280,-80 Q 197,-80 138.5,-138.5 80,-197 80,-280 80,-363 138.5,-421.5 197,-480 280,-480 q 83,0 141.5,58.5 58.5,58.5 58.5,141.5 0,83 -58.5,141.5 Q 363,-80 280,-80 Z M 824,-120 568,-376 Q 556,-389 542.5,-402.5 529,-416 516,-428 q 38,-24 61,-64 23,-40 23,-88 0,-75 -52.5,-127.5 Q 495,-760 420,-760 345,-760 292.5,-707.5 240,-655 240,-580 q 0,6 0.5,11.5 0.5,5.5 1.5,11.5 -18,2 -39.5,8 -21.5,6 -38.5,14 -2,-11 -3,-22 -1,-11 -1,-23 0,-109 75.5,-184.5 Q 311,-840 420,-840 q 109,0 184.5,75.5 75.5,75.5 75.5,184.5 0,43 -13.5,81.5 Q 653,-460 629,-428 l 251,252 z m -615,-61 71,-71 70,71 29,-28 -71,-71 71,-71 -28,-28 -71,71 -71,-71 -28,28 71,71 -71,71 z"/></svg>
<div class="mobile-wallet-adapter-embedded-modal-title">We can't find a wallet.</div>
<div id="mobile-wallet-adapter-error-message" class="mobile-wallet-adapter-embedded-modal-subtitle"></div>
<div>
    <button data-error-action id="mobile-wallet-adapter-error-action" class="mobile-wallet-adapter-embedded-modal-error-action">
        Find a wallet
    </button>
</div>
`,u_=`
.mobile-wallet-adapter-embedded-modal-content {
    text-align: center;
}

.mobile-wallet-adapter-embedded-modal-error-icon {
    margin-top: 24px;
}

.mobile-wallet-adapter-embedded-modal-title {
    margin: 18px 100px auto 100px;
    color: #000000;
    font-size: 2.75em;
    font-weight: 600;
}

.mobile-wallet-adapter-embedded-modal-subtitle {
    margin: 30px 60px 40px 60px;
    color: #000000;
    font-size: 1.25em;
    font-weight: 400;
}

.mobile-wallet-adapter-embedded-modal-error-action {
    display: block;
    width: 100%;
    height: 56px;
    /*margin-top: 40px;*/
    font-size: 1.25em;
    /*line-height: 24px;*/
    /*letter-spacing: -1%;*/
    background: #000000;
    color: #FFFFFF;
    border-radius: 18px;
}

/* Smaller screens */
@media all and (max-width: 600px) {
    .mobile-wallet-adapter-embedded-modal-title {
        font-size: 1.5em;
        margin-right: 12px;
        margin-left: 12px;
    }
    .mobile-wallet-adapter-embedded-modal-subtitle {
        margin-right: 12px;
        margin-left: 12px;
    }
}
`;async function R_(){if(typeof window<"u"){let t=window.navigator.userAgent.toLowerCase(),e=new d_;t.includes("wv")?e.initWithError({name:"SolanaMobileWalletAdapterError",code:"ERROR_BROWSER_NOT_SUPPORTED",message:""}):e.initWithError({name:"SolanaMobileWalletAdapterError",code:"ERROR_WALLET_NOT_FOUND",message:""}),e.open()}}function Zu(){return async()=>{R_()}}var E_=class extends he{contentStyles=A_;contentHtml=h_;initWithCallback(t){super.init(),this.#e(t)}#e(t){let e=this.dom?.getElementById("mobile-wallet-adapter-launch-action"),n=async()=>{e?.removeEventListener("click",n),this.close(),t()};e?.addEventListener("click",n)}},h_=`
<svg class="mobile-wallet-adapter-embedded-modal-launch-icon" width="48" height="48" viewBox="0 0 48 48" fill="none" xmlns="http://www.w3.org/2000/svg">
    <path d="M21.6 48C7.2 48 0 40.8 0 26.4V21.6C0 7.2 7.2 0 21.6 0H26.4C40.8 0 48 7.2 48 21.6V26.4C48 40.8 40.8 48 26.4 48H21.6Z" fill="#15994E"/>
    <mask id="mask0_189_522" style="mask-type:alpha" maskUnits="userSpaceOnUse" x="8" y="8" width="32" height="32">
        <rect x="8" y="8" width="32" height="32" fill="#D9D9D9"/>
    </mask>
    <g mask="url(#mask0_189_522)">
        <mask id="mask1_189_522" style="mask-type:alpha" maskUnits="userSpaceOnUse" x="8" y="8" width="32" height="32">
            <rect x="8" y="8" width="32" height="32" fill="#D9D9D9"/>
        </mask>
        <g mask="url(#mask1_189_522)">
            <path d="M22.1092 26.1208L19.4498 23.4615C19.1736 23.1851 18.8253 23.0468 18.4048 23.0468C17.9846 23.0468 17.6363 23.1851 17.3598 23.4615C17.0836 23.7377 16.9468 24.0861 16.9495 24.5065C16.9522 24.9267 17.0916 25.275 17.3678 25.5512L21.0405 29.2238C21.3463 29.5276 21.7031 29.6795 22.1108 29.6795C22.5184 29.6795 22.8742 29.5276 23.1782 29.2238L30.5918 21.8098C30.8683 21.5336 31.0065 21.1867 31.0065 20.7692C31.0065 20.3514 30.8683 20.0044 30.5918 19.7282C30.3156 19.4517 29.9673 19.3135 29.5468 19.3135C29.1266 19.3135 28.7784 19.4517 28.5022 19.7282L22.1092 26.1208ZM23.9998 37.6042C22.113 37.6042 20.3425 37.2473 18.6885 36.5335C17.0343 35.8197 15.5954 34.8512 14.3718 33.6278C13.1485 32.4043 12.18 30.9654 11.4662 29.3112C10.7524 27.6572 10.3955 25.8867 10.3955 23.9998C10.3955 22.113 10.7524 20.3425 11.4662 18.6885C12.18 17.0343 13.1485 15.5954 14.3718 14.3718C15.5954 13.1485 17.0343 12.18 18.6885 11.4662C20.3425 10.7524 22.113 10.3955 23.9998 10.3955C25.8867 10.3955 27.6572 10.7524 29.3112 11.4662C30.9654 12.18 32.4043 13.1485 33.6278 14.3718C34.8512 15.5954 35.8197 17.0343 36.5335 18.6885C37.2473 20.3425 37.6042 22.113 37.6042 23.9998C37.6042 25.8867 37.2473 27.6572 36.5335 29.3112C35.8197 30.9654 34.8512 32.4043 33.6278 33.6278C32.4043 34.8512 30.9654 35.8197 29.3112 36.5335C27.6572 37.2473 25.8867 37.6042 23.9998 37.6042Z" fill="white"/>
        </g>
    </g>
</svg>
<div class="mobile-wallet-adapter-embedded-modal-title">Ready to connect!</div>
<div>
    <button data-modal-action id="mobile-wallet-adapter-launch-action" class="mobile-wallet-adapter-embedded-modal-launch-action">
        Connect Wallet
    </button>
</div>
`,A_=`
.mobile-wallet-adapter-embedded-modal-close {
    display: none;
}
.mobile-wallet-adapter-embedded-modal-content {
    text-align: center;
    min-width: 300px;
}
.mobile-wallet-adapter-embedded-modal-launch-icon {
    margin-top: 24px;
}
.mobile-wallet-adapter-embedded-modal-title {
    margin: 18px 100px 30px 100px;
    color: #000000;
    font-size: 2.75em;
    font-weight: 600;
}
.mobile-wallet-adapter-embedded-modal-launch-action {
    display: block;
    width: 100%;
    height: 56px;
    font-size: 1.25em;
    background: #000000;
    color: #FFFFFF;
    border-radius: 18px;
}
/* Smaller screens */
@media all and (max-width: 600px) {
    .mobile-wallet-adapter-embedded-modal-title {
        font-size: 1.5em;
        margin-right: 12px;
        margin-left: 12px;
    }
}
`,O_=class extends he{contentStyles=S_;get contentHtml(){let t=C_()?"Long press the app icon on your home screen to open site settings":"Tap the lock or settings icon in the address bar to open site settings";return N_.replace("{{PERMISSION_INSTRUCTION_DETAIL}}",t)}async init(){super.init(),this.#e()}#e(){let t=this.dom?.getElementById("mobile-wallet-adapter-launch-action"),e=async n=>{t?.removeEventListener("click",e),this.close(n)};t?.addEventListener("click",e)}},N_=`
<div class="mobile-wallet-adapter-embedded-modal-header">
    Local Wallet Connection
</div>
<svg width="48" height="48" viewBox="0 0 48 48" fill="none" xmlns="http://www.w3.org/2000/svg">
    <path d="M21.6 48C7.2 48 0 40.8 0 26.4V21.6C0 7.2 7.2 0 21.6 0H26.4C40.8 0 48 7.2 48 21.6V26.4C48 40.8 40.8 48 26.4 48H21.6Z" fill="#ED1515"/>
    <mask id="mask0_147_1364" style="mask-type:alpha" maskUnits="userSpaceOnUse" x="8" y="8" width="32" height="32">
        <rect x="8" y="8" width="32" height="32" fill="#D9D9D9"/>
    </mask>
    <g mask="url(#mask0_147_1364)">
        <path d="M20.1398 36.2705C19.7363 36.2705 19.3508 36.1945 18.9835 36.0425C18.6162 35.8907 18.2916 35.674 18.0098 35.3922L12.6072 29.9895C12.3254 29.7077 12.1086 29.3832 11.9568 29.0158C11.8048 28.6485 11.7288 28.2631 11.7288 27.8595V20.1395C11.7288 19.736 11.8048 19.3505 11.9568 18.9832C12.1086 18.6158 12.3254 18.2913 12.6072 18.0095L18.0098 12.6068C18.2916 12.3251 18.6162 12.1083 18.9835 11.9565C19.3508 11.8045 19.7363 11.7285 20.1398 11.7285H27.8598C28.2634 11.7285 28.6488 11.8045 29.0162 11.9565C29.3835 12.1083 29.708 12.3251 29.9898 12.6068L35.3925 18.0095C35.6743 18.2913 35.891 18.6158 36.0428 18.9832C36.1948 19.3505 36.2708 19.736 36.2708 20.1395V27.8595C36.2708 28.2631 36.1948 28.6485 36.0428 29.0158C35.891 29.3832 35.6743 29.7077 35.3925 29.9895L29.9898 35.3922C29.708 35.674 29.3835 35.8907 29.0162 36.0425C28.6488 36.1945 28.2634 36.2705 27.8598 36.2705H20.1398ZM20.1732 33.2372H27.8265L33.2375 27.8262V20.1728L27.8265 14.7618H20.1732L14.7622 20.1728V27.8262L20.1732 33.2372ZM23.9998 25.9538L26.7868 28.7408C27.0473 29.0013 27.3729 29.1302 27.7638 29.1275C28.1549 29.1248 28.4807 28.9933 28.7412 28.7328C29.0016 28.4724 29.1318 28.1466 29.1318 27.7555C29.1318 27.3646 29.0016 27.039 28.7412 26.7785L25.9542 23.9995L28.7412 21.2125C29.0016 20.9521 29.1318 20.6264 29.1318 20.2355C29.1318 19.8444 29.0016 19.5186 28.7412 19.2582C28.4807 18.9977 28.1549 18.8675 27.7638 18.8675C27.3729 18.8675 27.0473 18.9977 26.7868 19.2582L23.9998 22.0452L21.2128 19.2582C20.9524 18.9977 20.628 18.8675 20.2398 18.8675C19.8514 18.8675 19.5269 18.9977 19.2665 19.2582C19.006 19.5186 18.8758 19.8444 18.8758 20.2355C18.8758 20.6264 19.006 20.9521 19.2665 21.2125L22.0455 23.9995L19.2585 26.7865C18.998 27.047 18.8692 27.3713 18.8718 27.7595C18.8745 28.148 19.006 28.4724 19.2665 28.7328C19.5269 28.9933 19.8527 29.1235 20.2438 29.1235C20.6347 29.1235 20.9604 28.9933 21.2208 28.7328L23.9998 25.9538Z" fill="black"/>
    </g>
</svg>
<div class="mobile-wallet-adapter-embedded-modal-title">
    Your wallet connection is blocked
</div>
<div id="mobile-wallet-adapter-local-launch-message" class="mobile-wallet-adapter-embedded-modal-subtitle">
    Visit site settings in the address bar and allow "Apps on Device".
</div>

<div class="mobile-wallet-adapter-embedded-modal-divider"><hr></div>
<div class="mobile-wallet-adapter-embedded-modal-footer">
    <div class="mobile-wallet-adapter-embedded-modal-details">
        <!-- Clickable header (label associated with the checkbox) -->
      	<label for="collapsible-1" class="mobile-wallet-adapter-embedded-modal-details-collapsible-header">
            <!-- Hidden checkbox to track state -->
            <input type="checkbox" id="collapsible-1" class="mobile-wallet-adapter-embedded-modal-details-collapsible-input">
            <span class="mobile-wallet-adapter-embedded-modal-details-collapsible-header-label">
              See details
            </span>
            <svg class="mobile-wallet-adapter-embedded-modal-details-collapsible-header-icon" width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
              <mask id="mask0_147_1382" style="mask-type:alpha" maskUnits="userSpaceOnUse" x="0" y="0" width="24" height="24">
                <rect width="24" height="24" fill="#D9D9D9"/>
              </mask>
              <g mask="url(#mask0_147_1382)">
                <path d="M11.9999 17.0811C11.8506 17.0811 11.7087 17.0563 11.5741 17.0067C11.4395 16.957 11.3162 16.8762 11.2042 16.7643L6.57924 12.1393C6.36801 11.9281 6.26656 11.667 6.27489 11.3561C6.28322 11.0453 6.39301 10.7842 6.60424 10.573C6.81547 10.3618 7.08069 10.2561 7.39989 10.2561C7.71909 10.2561 7.9843 10.3618 8.19554 10.573L11.9999 14.3773L15.8292 10.548C16.0405 10.3368 16.3015 10.2353 16.6124 10.2436C16.9233 10.252 17.1843 10.3618 17.3955 10.573C17.6068 10.7842 17.7124 11.0494 17.7124 11.3686C17.7124 11.6878 17.6068 11.9531 17.3955 12.1643L12.7955 16.7643C12.6836 16.8762 12.5603 16.957 12.4257 17.0067C12.2911 17.0563 12.1492 17.0811 11.9999 17.0811Z" fill="black"/>
              </g>
            </svg>
      	</label>
        
        <!-- Content to show/hide -->
        <ul class="mobile-wallet-adapter-embedded-modal-details-collapsible-content">
            <li>{{PERMISSION_INSTRUCTION_DETAIL}}</li>
            <li>Allow "Apps on Device"</li>
        </ul>
    </div>
</div>
<div>
    <button data-modal-action id="mobile-wallet-adapter-launch-action" class="mobile-wallet-adapter-embedded-modal-launch-action">
        Got it
    </button>
</div>
`,S_=`
.mobile-wallet-adapter-embedded-modal-close {
    display: none;
}
.mobile-wallet-adapter-embedded-modal-content {
    text-align: center;
}
.mobile-wallet-adapter-embedded-modal-header {
    margin: 18px auto 30px auto;
    color: #7D9093;
    font-size: 1.0em;
    font-weight: 500;
}
.mobile-wallet-adapter-embedded-modal-title {
    margin: 18px 100px auto 100px;
    color: #000000;
    font-size: 2.75em;
    font-weight: 600;
}
.mobile-wallet-adapter-embedded-modal-subtitle {
    margin: 12px 60px 30px 60px;
    color: #7D9093;
    font-size: 1.25em;
    font-weight: 400;
}
.mobile-wallet-adapter-embedded-modal-details-collapsible-header {
    display: flex;
    flex-direction: row;
  	justify-content: space-between;
    margin: 10px auto 10px auto;
    color: #000000;
    font-size: 1.5em;
    font-weight: 600;
    cursor: pointer; /* Show pointer on hover */
    transition: background 0.2s ease; /* Smooth background change */
}
.mobile-wallet-adapter-embedded-modal-details-collapsible-header-icon {
  	transition: rotate 0.3s ease;
}
.mobile-wallet-adapter-embedded-modal-details-collapsible-input {
  	display: none; /* Hide the checkbox */
}
.mobile-wallet-adapter-embedded-modal-details-collapsible-content {
    margin: 0px auto 40px auto;
    max-height: 0px; /* Collapse content */
    overflow: hidden; /* Hide overflow when collapsed */
    transition: max-height 0.3s ease; /* Smooth transition */
}
.mobile-wallet-adapter-embedded-modal-details-collapsible-content li {
    margin: 20px auto;
    color: #000000;
    font-size: 1.25em;
    font-weight: 400;
    text-align: left;
}
/* When checkbox is checked, show content */
.mobile-wallet-adapter-embedded-modal-details-collapsible-header:has(> input:checked) ~ .mobile-wallet-adapter-embedded-modal-details-collapsible-content {
  	max-height: 300px;
}
.mobile-wallet-adapter-embedded-modal-details-collapsible-header:has(> input:checked) > .mobile-wallet-adapter-embedded-modal-details-collapsible-header-icon {
  	rotate: 180deg;
}
.mobile-wallet-adapter-embedded-modal-launch-action {
    display: block;
    width: 100%;
    height: 56px;
    /*margin-top: 40px;*/
    font-size: 1.25em;
    /*line-height: 24px;*/
    /*letter-spacing: -1%;*/
    background: #000000;
    color: #FFFFFF;
    border-radius: 18px;
}
/* Smaller screens */
@media all and (max-width: 600px) {
    .mobile-wallet-adapter-embedded-modal-title {
        font-size: 1.75em;
        margin-right: 12px;
        margin-left: 12px;
    }
    .mobile-wallet-adapter-embedded-modal-subtitle {
        margin-right: 12px;
        margin-left: 12px;
    }
}
`,m_=class extends he{contentStyles=g_;contentHtml=p_;async init(){super.init(),this.#e()}#e(){let t=this.dom?.getElementById("mobile-wallet-adapter-launch-action"),e=async()=>{t?.removeEventListener("click",e);try{await fetch("http://localhost")}catch{}this.close()};t?.addEventListener("click",e)}},p_=`
<div class="mobile-wallet-adapter-embedded-modal-title">Allow connections to your wallet</div>
<div id="mobile-wallet-adapter-local-launch-message" class="mobile-wallet-adapter-embedded-modal-subtitle">
    Tap "Allow" on the next screen
</div>
<svg class="mobile-wallet-adapter-embedded-modal-permission-prompt-mock" xmlns="http://www.w3.org/2000/svg" width="281" height="83" viewBox="0 0 281 83" fill="none">
    <rect width="281" height="83" rx="22" fill="#F0F3F5"/>
    <path d="M254.194 64L252.626 56.657H254.047L254.866 61.452L254.985 62.278H255.02L255.146 61.452L255.993 57.497H257.4L258.254 61.431L258.373 62.278H258.415L258.534 61.431L259.346 56.657H260.718L259.143 64H257.673L256.826 59.961L256.693 59.093H256.651L256.511 59.961L255.664 64H254.194Z" fill="black"/>
    <path d="M248.837 64.231C248.147 64.231 247.54 64.07 247.017 63.748C246.495 63.426 246.086 62.978 245.792 62.404C245.498 61.83 245.351 61.1673 245.351 60.416V60.241C245.351 59.4897 245.498 58.827 245.792 58.253C246.086 57.679 246.495 57.2333 247.017 56.916C247.54 56.594 248.147 56.433 248.837 56.433C249.528 56.433 250.135 56.594 250.657 56.916C251.18 57.2333 251.588 57.679 251.882 58.253C252.176 58.827 252.323 59.4897 252.323 60.241V60.416C252.323 61.1673 252.176 61.83 251.882 62.404C251.588 62.978 251.18 63.426 250.657 63.748C250.135 64.07 249.528 64.231 248.837 64.231ZM248.837 62.824C249.43 62.824 249.897 62.607 250.237 62.173C250.583 61.7343 250.755 61.1417 250.755 60.395V60.262C250.755 59.5107 250.583 58.918 250.237 58.484C249.897 58.05 249.43 57.833 248.837 57.833C248.249 57.833 247.783 58.05 247.437 58.484C247.092 58.918 246.919 59.5107 246.919 60.262V60.395C246.919 61.1417 247.092 61.7343 247.437 62.173C247.783 62.607 248.249 62.824 248.837 62.824Z" fill="black"/>
    <path d="M242.298 64.231C241.467 64.231 240.814 63.993 240.338 63.517C239.866 63.0364 239.631 62.3737 239.631 61.529V53.78H241.178V61.389C241.178 62.3317 241.591 62.803 242.417 62.803C242.65 62.803 242.865 62.7587 243.061 62.67C243.257 62.5814 243.464 62.4367 243.684 62.236L244.538 63.377C244.225 63.6664 243.884 63.881 243.516 64.021C243.152 64.161 242.746 64.231 242.298 64.231ZM237.51 55.061V53.78H240.611V55.061H237.51Z" fill="black"/>
    <path d="M234.463 64.231C233.633 64.231 232.979 63.993 232.503 63.517C232.032 63.0364 231.796 62.3737 231.796 61.529V53.78H233.343V61.389C233.343 62.3317 233.756 62.803 234.582 62.803C234.816 62.803 235.03 62.7587 235.226 62.67C235.422 62.5814 235.63 62.4367 235.849 62.236L236.703 63.377C236.391 63.6664 236.05 63.881 235.681 64.021C235.317 64.161 234.911 64.231 234.463 64.231ZM229.675 55.061V53.78H232.776V55.061H229.675Z" fill="black"/>
    <path d="M221.442 64L224.557 53.976H226.132L229.233 64H227.581L225.642 56.972L225.341 55.761H225.299L225.005 56.972L223.073 64H221.442ZM222.835 61.634L223.255 60.29H227.371L227.805 61.634H222.835Z" fill="black"/>
    <path d="M178.261 64L175.034 60.066V60.024L178.121 56.657H180.011L176.504 60.423V59.632L180.165 64H178.261ZM173.543 64V53.78H175.097V64H173.543Z" fill="#7D9093" fill-opacity="0.5"/>
    <path d="M169.306 64.224C168.588 64.224 167.958 64.0653 167.416 63.748C166.88 63.426 166.462 62.9803 166.163 62.411C165.865 61.837 165.715 61.1673 165.715 60.402V60.248C165.715 59.4873 165.862 58.8223 166.156 58.253C166.45 57.679 166.863 57.2333 167.395 56.916C167.927 56.594 168.546 56.433 169.25 56.433C169.978 56.433 170.59 56.6056 171.084 56.951C171.579 57.2917 171.955 57.777 172.211 58.407L170.874 58.995C170.72 58.6123 170.508 58.323 170.237 58.127C169.967 57.9263 169.633 57.826 169.236 57.826C168.63 57.826 168.149 58.0383 167.794 58.463C167.444 58.883 167.269 59.4616 167.269 60.199V60.465C167.269 61.1837 167.454 61.7577 167.822 62.187C168.196 62.6163 168.69 62.831 169.306 62.831C169.712 62.831 170.06 62.733 170.349 62.537C170.639 62.341 170.877 62.0423 171.063 61.641L172.379 62.285C172.188 62.6957 171.941 63.0457 171.637 63.335C171.334 63.6243 170.986 63.846 170.594 64C170.202 64.1493 169.773 64.224 169.306 64.224Z" fill="#7D9093" fill-opacity="0.5"/>
    <path d="M161.003 64.231C160.312 64.231 159.706 64.07 159.183 63.748C158.66 63.426 158.252 62.978 157.958 62.404C157.664 61.83 157.517 61.1673 157.517 60.416V60.241C157.517 59.4897 157.664 58.827 157.958 58.253C158.252 57.679 158.66 57.2333 159.183 56.916C159.706 56.594 160.312 56.433 161.003 56.433C161.694 56.433 162.3 56.594 162.823 56.916C163.346 57.2333 163.754 57.679 164.048 58.253C164.342 58.827 164.489 59.4897 164.489 60.241V60.416C164.489 61.1673 164.342 61.83 164.048 62.404C163.754 62.978 163.346 63.426 162.823 63.748C162.3 64.07 161.694 64.231 161.003 64.231ZM161.003 62.824C161.596 62.824 162.062 62.607 162.403 62.173C162.748 61.7343 162.921 61.1417 162.921 60.395V60.262C162.921 59.5107 162.748 58.918 162.403 58.484C162.062 58.05 161.596 57.833 161.003 57.833C160.415 57.833 159.948 58.05 159.603 58.484C159.258 58.918 159.085 59.5107 159.085 60.262V60.395C159.085 61.1417 159.258 61.7343 159.603 62.173C159.948 62.607 160.415 62.824 161.003 62.824Z" fill="#7D9093" fill-opacity="0.5"/>
    <path d="M154.463 64.231C153.633 64.231 152.979 63.993 152.503 63.517C152.032 63.0364 151.796 62.3737 151.796 61.529V53.78H153.343V61.389C153.343 62.3317 153.756 62.803 154.582 62.803C154.816 62.803 155.03 62.7587 155.226 62.67C155.422 62.5814 155.63 62.4367 155.849 62.236L156.703 63.377C156.391 63.6664 156.05 63.881 155.681 64.021C155.317 64.161 154.911 64.231 154.463 64.231ZM149.675 55.061V53.78H152.776V55.061H149.675Z" fill="#7D9093" fill-opacity="0.5"/>
    <path d="M142.24 64V53.976H145.544C146.421 53.976 147.112 54.1953 147.616 54.634C148.12 55.0726 148.372 55.6583 148.372 56.391V56.566C148.372 57.0886 148.246 57.5366 147.994 57.91C147.742 58.2833 147.38 58.5586 146.909 58.736V58.792C147.492 58.9226 147.947 59.2003 148.274 59.625C148.605 60.045 148.771 60.5606 148.771 61.172V61.361C148.771 61.893 148.645 62.3573 148.393 62.754C148.145 63.1506 147.795 63.4586 147.343 63.678C146.895 63.8926 146.365 64 145.754 64H142.24ZM143.794 62.656H145.572C146.085 62.656 146.482 62.5253 146.762 62.264C147.042 62.0026 147.182 61.6293 147.182 61.144V60.99C147.182 60.5046 147.037 60.1313 146.748 59.87C146.463 59.604 146.05 59.471 145.509 59.471H143.36V58.183H145.32C145.791 58.183 146.153 58.064 146.405 57.826C146.657 57.588 146.783 57.2496 146.783 56.811V56.685C146.783 56.2416 146.657 55.9033 146.405 55.67C146.157 55.4366 145.796 55.32 145.32 55.32H143.794V62.656Z" fill="#7D9093" fill-opacity="0.5"/>
    <rect x="18" y="17" width="246" height="7" rx="3.5" fill="#7D9093" fill-opacity="0.26"/>
    <rect x="18" y="33" width="82" height="7" rx="3.5" fill="#7D9093" fill-opacity="0.26"/>
</svg>
<div>
    <button data-modal-action id="mobile-wallet-adapter-launch-action" class="mobile-wallet-adapter-embedded-modal-launch-action">
        Continue to Allow
    </button>
</div>
`,g_=`
.mobile-wallet-adapter-embedded-modal-close {
    display: none;
}
.mobile-wallet-adapter-embedded-modal-content {
    text-align: center;
}
.mobile-wallet-adapter-embedded-modal-title {
    margin: 18px 100px auto 100px;
    color: #000000;
    font-size: 2.75em;
    font-weight: 600;
}
.mobile-wallet-adapter-embedded-modal-subtitle {
    margin: 20px 60px 40px 60px;
    color: #7D9093;
    font-size: 1.25em;
    font-weight: 400;
}
.mobile-wallet-adapter-embedded-modal-permission-prompt-mock {
    width: 90%;
    height: auto;
    margin: 0 auto 30px auto;
    display: block;
}
.mobile-wallet-adapter-embedded-modal-launch-action {
    display: block;
    width: 100%;
    height: 56px;
    font-size: 1.25em;
    background: #000000;
    color: #FFFFFF;
    border-radius: 18px;
}
/* Smaller screens */
@media all and (max-width: 600px) {
    .mobile-wallet-adapter-embedded-modal-title {
        font-size: 1.5em;
        margin-right: 12px;
        margin-left: 12px;
    }
    .mobile-wallet-adapter-embedded-modal-subtitle {
        margin-right: 12px;
        margin-left: 12px;
    }
}
`;function f_(){return typeof window<"u"&&window.isSecureContext&&typeof document<"u"&&/android/i.test(navigator.userAgent)}function T_(){return typeof window<"u"&&window.isSecureContext&&typeof document<"u"&&!/Android|webOS|iPhone|iPad|iPod|BlackBerry|IEMobile|Opera Mini/i.test(navigator.userAgent)}function I_(t){return/(WebView|Version\/.+(Chrome)\/(\d+)\.(\d+)\.(\d+)\.(\d+)|; wv\).+(Chrome)\/(\d+)\.(\d+)\.(\d+)\.(\d+))/i.test(t)}function sa(t){return t.includes("Solana Mobile Web Shell")}function C_(){let t=typeof document<"u"&&document.referrer.startsWith("android-app://");if(typeof window>"u")return t;let e=window.matchMedia("(display-mode: standalone)").matches,n=window.matchMedia("(display-mode: fullscreen)").matches,a=window.matchMedia("(display-mode: minimal-ui)").matches;return t||e||n||a}async function ca(){if(!(typeof navigator<"u"&&sa(navigator.userAgent)))try{let t=await navigator.permissions.query({name:"loopback-network"});if(t.state==="granted")return;if(t.state==="denied"){let e=new O_;throw e.init(),e.open(),new O(S.ERROR_LOOPBACK_ACCESS_BLOCKED,"Local Network Access permission denied")}else if(t.state==="prompt"){let e=new m_;if(await new Promise((n,a)=>{e.addEventListener("close",r=>{r&&a(new O(S.ERROR_ASSOCIATION_CANCELLED,"Wallet connection cancelled by user",{event:r}))}),t.onchange=()=>{t.onchange=null,n(t.state)},e.init(),e.open()})==="granted"){let n=new E_;await new Promise((a,r)=>{n.addEventListener("close",o=>{o&&r(new O(S.ERROR_ASSOCIATION_CANCELLED,"Wallet connection cancelled by user",{event:o}))}),n.initWithCallback(async()=>{a(!0)}),n.open()});return}else return await ca()}throw new O(S.ERROR_LOOPBACK_ACCESS_BLOCKED,"Local Network Access permission unknown")}catch(t){if(t instanceof TypeError&&(t.message.includes("loopback-network")||t.message.includes("local-network-access")))return;throw t instanceof O?t:new O(S.ERROR_LOOPBACK_ACCESS_BLOCKED,t instanceof Error?t.message:"Local Network Access permission unknown")}}var w_=`
<div class="mobile-wallet-adapter-embedded-loading-indicator" role="dialog" aria-modal="true" aria-labelledby="modal-title">
    <div data-modal-close style="position: absolute; width: 100%; height: 100%;"></div>
    <div class="mobile-wallet-adapter-embedded-loading-container">
        <div class="mobile-wallet-adapter-embedded-loading-animation"></div>
    </div>
</div>
`,L_=`
.mobile-wallet-adapter-embedded-loading-indicator {
    display: flex; /* Use flexbox to center content */
    justify-content: center; /* Center horizontally */
    align-items: start; /* Center vertically */
    position: fixed; /* Stay in place */
    z-index: 1; /* Sit on top */
    left: 0;
    top: 0;
    width: 100%; /* Full width */
    height: 100%; /* Full height */
    background-color: rgba(0,0,0,0.4); /* Black w/ opacity */
    overflow-y: auto; /* enable scrolling */
}

.mobile-wallet-adapter-embedded-loading-container {
    display: flex;
    margin: auto;
}

.mobile-wallet-adapter-embedded-loading-animation {
    position: relative;
    left: -9999px;
    width: 10px;
    height: 10px;
    border-radius: 5px;
    background-color: var(--spinner-color);
    color: var(--spinner-color);
    box-shadow: 9984px 0 0 0 var(--spinner-color), 
                9999px 0 0 0 var(--spinner-color), 
                10014px 0 0 0 var(--spinner-color);
    animation: dot-typing 1.5s infinite linear;
}

@keyframes dot-typing {
    0% {
        box-shadow: 9984px 0 0 0 var(--spinner-color), 
                    9999px 0 0 0 var(--spinner-color), 
                    10014px 0 0 0 var(--spinner-color);
    }
    16.667% {
        box-shadow: 9984px -10px 0 0 var(--spinner-color), 
                    9999px 0 0 0 var(--spinner-color), 
                    10014px 0 0 0 var(--spinner-color);
    }
    33.333% {
        box-shadow: 9984px 0 0 0 var(--spinner-color), 
                    9999px 0 0 0 var(--spinner-color), 
                    10014px 0 0 0 var(--spinner-color);
    }
    50% {
        box-shadow: 9984px 0 0 0 var(--spinner-color), 
                    9999px -10px 0 0 var(--spinner-color), 
                    10014px 0 0 0 var(--spinner-color);
    }
    66.667% {
        box-shadow: 9984px 0 0 0 var(--spinner-color), 
                    9999px 0 0 0 var(--spinner-color), 
                    10014px 0 0 0 var(--spinner-color);
    }
    83.333% {
        box-shadow: 9984px 0 0 0 var(--spinner-color), 
                    9999px 0 0 0 var(--spinner-color), 
                    10014px -10px 0 0 var(--spinner-color);
    }
    100% {
        box-shadow: 9984px 0 0 0 var(--spinner-color), 
                    9999px 0 0 0 var(--spinner-color), 
                    10014px 0 0 0 var(--spinner-color);
    }
}
`,b_=class{#e=null;#n={};#r=!1;dom=null;constructor(){this.init=this.init.bind(this),this.#e=document.getElementById("mobile-wallet-adapter-embedded-root-ui")}async init(){console.log("Injecting modal"),this.#d()}open=()=>{console.debug("Modal open"),this.#_(),this.#e&&(this.#e.style.display="flex")};close=(t=void 0)=>{console.debug("Modal close"),this.#s(),this.#e&&(this.#e.style.display="none"),this.#n.close?.forEach(e=>e(t))};addEventListener(t,e){return this.#n[t]?.push(e)||(this.#n[t]=[e]),()=>this.removeEventListener(t,e)}removeEventListener(t,e){this.#n[t]=this.#n[t]?.filter(n=>e!==n)}#d(){if(this.dom)return;this.#e=document.createElement("div"),this.#e.id="mobile-wallet-adapter-embedded-root-ui",this.#e.innerHTML=w_,this.#e.style.display="none";let t=document.createElement("style");t.id="mobile-wallet-adapter-embedded-modal-styles",t.textContent=L_;let e=document.createElement("div");this.dom=e.attachShadow({mode:"closed"}),e.style.setProperty("--spinner-color","#FFFFFF"),this.dom.appendChild(t),this.dom.appendChild(this.#e),document.body.appendChild(e)}#_(){!this.#e||this.#r||([...this.#e.querySelectorAll("[data-modal-close]")].forEach(t=>t?.addEventListener("click",e=>{this.close(e)})),window.addEventListener("load",this.close),document.addEventListener("keydown",this.#t),this.#r=!0)}#s(){this.#r&&(window.removeEventListener("load",this.close),document.removeEventListener("keydown",this.#t),this.#e&&([...this.#e.querySelectorAll("[data-modal-close]")].forEach(t=>t?.removeEventListener("click",this.close)),this.#r=!1))}#t=t=>{t.key==="Escape"&&this.close(t)}},y_=class extends he{contentStyles=D_;contentHtml=v_;async initWithQR(t){super.init(),this.populateQRCode(t)}async populateQRCode(t){let e=this.dom?.getElementById("mobile-wallet-adapter-embedded-modal-qr-code-container");if(e){let n=await ia.default.toCanvas(t,{width:200,margin:0});e.firstElementChild!==null?e.replaceChild(n,e.firstElementChild):e.appendChild(n);let a=this.dom?.getElementById("mobile-wallet-adapter-embedded-modal-qr-placeholder");a&&(a.style.display="none")}else console.error("QRCode Container not found")}},v_=`
<div class="mobile-wallet-adapter-embedded-modal-qr-content">
    <div>
        <svg class="mobile-wallet-adapter-embedded-modal-icon" width="100%" height="100%">
            <circle r="52" cx="53" cy="53" fill="#99b3be" stroke="#000000" stroke-width="2"/>
            <path d="m 53,82.7305 c -3.3116,0 -6.1361,-1.169 -8.4735,-3.507 -2.338,-2.338 -3.507,-5.1625 -3.507,-8.4735 0,-3.3116 1.169,-6.1364 3.507,-8.4744 2.3374,-2.338 5.1619,-3.507 8.4735,-3.507 3.3116,0 6.1361,1.169 8.4735,3.507 2.338,2.338 3.507,5.1628 3.507,8.4744 0,3.311 -1.169,6.1355 -3.507,8.4735 -2.3374,2.338 -5.1619,3.507 -8.4735,3.507 z m 0.007,-5.25 c 1.8532,0 3.437,-0.6598 4.7512,-1.9793 1.3149,-1.3195 1.9723,-2.9058 1.9723,-4.7591 0,-1.8526 -0.6598,-3.4364 -1.9793,-4.7512 -1.3195,-1.3149 -2.9055,-1.9723 -4.7582,-1.9723 -1.8533,0 -3.437,0.6598 -4.7513,1.9793 -1.3148,1.3195 -1.9722,2.9058 -1.9722,4.7591 0,1.8527 0.6597,3.4364 1.9792,4.7512 1.3195,1.3149 2.9056,1.9723 4.7583,1.9723 z m -28,-33.5729 -3.85,-3.6347 c 4.1195,-4.025 8.8792,-7.1984 14.2791,-9.52 5.4005,-2.3223 11.2551,-3.4834 17.5639,-3.4834 6.3087,0 12.1634,1.1611 17.5639,3.4834 5.3999,2.3216 10.1596,5.495 14.2791,9.52 l -3.85,3.6347 C 77.2999,40.358 73.0684,37.5726 68.2985,35.5514 63.5292,33.5301 58.4296,32.5195 53,32.5195 c -5.4297,0 -10.5292,1.0106 -15.2985,3.0319 -4.7699,2.0212 -9.0014,4.8066 -12.6945,8.3562 z m 44.625,10.8771 c -2.2709,-2.1046 -4.7962,-3.7167 -7.5758,-4.8361 -2.7795,-1.12 -5.7983,-1.68 -9.0562,-1.68 -3.2579,0 -6.2621,0.56 -9.0125,1.68 -2.7504,1.1194 -5.2903,2.7315 -7.6195,4.8361 L 32.5189,51.15 c 2.8355,-2.6028 5.9777,-4.6086 9.4263,-6.0174 3.4481,-1.4087 7.133,-2.1131 11.0548,-2.1131 3.9217,0 7.5979,0.7044 11.0285,2.1131 3.43,1.4088 6.5631,3.4146 9.3992,6.0174 z"/>
        </svg>
        <div class="mobile-wallet-adapter-embedded-modal-title">Remote Mobile Wallet Adapter</div>
    </div>
    <div>
        <div>
            <h4 class="mobile-wallet-adapter-embedded-modal-qr-label">
                Open your wallet and scan this code
            </h4>
        </div>
        <div id="mobile-wallet-adapter-embedded-modal-qr-code-container" class="mobile-wallet-adapter-embedded-modal-qr-code-container">
            <div id="mobile-wallet-adapter-embedded-modal-qr-placeholder" class="mobile-wallet-adapter-embedded-modal-qr-placeholder"></div>
        </div>
    </div>
</div>
<div class="mobile-wallet-adapter-embedded-modal-divider"><hr></div>
<div class="mobile-wallet-adapter-embedded-modal-footer">
    <div class="mobile-wallet-adapter-embedded-modal-subtitle">
        Follow the instructions on your device. When you're finished, this screen will update.
    </div>
    <div class="mobile-wallet-adapter-embedded-modal-progress-badge">
        <div>
            <div class="spinner">
                <div class="leftWrapper">
                    <div class="left">
                        <div class="circle"></div>
                    </div>
                </div>
                <div class="rightWrapper">
                    <div class="right">
                        <div class="circle"></div>
                    </div>
                </div>
            </div>
        </div>
        <div>Waiting for scan</div>
    </div>
</div>
`,D_=`
.mobile-wallet-adapter-embedded-modal-qr-content {
    display: flex; 
    margin-top: 10px;
    padding: 10px;
}

.mobile-wallet-adapter-embedded-modal-qr-content > div:first-child {
    display: flex;
    flex-direction: column;
    flex: 2;
    margin-top: auto;
    margin-right: 30px;
}

.mobile-wallet-adapter-embedded-modal-qr-content > div:nth-child(2) {
    display: flex;
    flex-direction: column;
    flex: 1;
    margin-left: auto;
}

.mobile-wallet-adapter-embedded-modal-footer {
    display: flex;
    padding: 10px;
}

.mobile-wallet-adapter-embedded-modal-icon {}

.mobile-wallet-adapter-embedded-modal-title {
    color: #000000;
    font-size: 2.5em;
    font-weight: 600;
}

.mobile-wallet-adapter-embedded-modal-qr-label {
    text-align: right;
    color: #000000;
}

.mobile-wallet-adapter-embedded-modal-qr-code-container {
    margin-left: auto;
}

.mobile-wallet-adapter-embedded-modal-qr-placeholder {
    margin-left: auto;
    min-width: 200px;
    min-height: 200px;
    background: linear-gradient(-60deg, #F7F8F8 30%, #ECEEEE 50%, #F7F8F8 70%);
    background-size: 200%;
    animation: placeholderAnimate 2.7s linear infinite;
    border-radius: 12px;
}

.mobile-wallet-adapter-embedded-modal-divider {
    margin-top: 20px;
    padding-left: 10px;
    padding-right: 10px;
}

.mobile-wallet-adapter-embedded-modal-divider hr {
    border-top: 1px solid #D9DEDE;
}

.mobile-wallet-adapter-embedded-modal-subtitle {
    margin: auto;
    margin-right: 60px;
    padding: 20px;
    color: #6E8286;
}

.mobile-wallet-adapter-embedded-modal-progress-badge {
    display: flex;
    background: #F7F8F8;
    height: 56px;
    min-width: 200px;
    margin: auto;
    padding-left: 20px;
    padding-right: 20px;
    border-radius: 18px;
    color: #A8B6B8;
    align-items: center;
}

.mobile-wallet-adapter-embedded-modal-progress-badge > div:first-child {
    margin-left: auto;
    margin-right: 20px;
}

.mobile-wallet-adapter-embedded-modal-progress-badge > div:nth-child(2) {
    margin-right: auto;
}

/* Smaller screens */
@media all and (max-width: 600px) {
    .mobile-wallet-adapter-embedded-modal-card {
        text-align: center;
    }
    .mobile-wallet-adapter-embedded-modal-qr-content {
        flex-direction: column;
    }
    .mobile-wallet-adapter-embedded-modal-qr-content > div:first-child {
        margin: auto;
    }
    .mobile-wallet-adapter-embedded-modal-qr-content > div:nth-child(2) {
        margin: auto;
        flex: 2 auto;
    }
    .mobile-wallet-adapter-embedded-modal-footer {
        flex-direction: column;
    }
    .mobile-wallet-adapter-embedded-modal-icon {
        display: none;
    }
    .mobile-wallet-adapter-embedded-modal-title {
        font-size: 1.5em;
    }
    .mobile-wallet-adapter-embedded-modal-subtitle {
        margin-right: unset;
    }
    .mobile-wallet-adapter-embedded-modal-qr-label {
        text-align: center;
    }
    .mobile-wallet-adapter-embedded-modal-qr-code-container {
        margin: auto;
    }
    .mobile-wallet-adapter-embedded-modal-qr-placeholder {
        margin: auto;
    }
}

/* QR Placeholder */
@keyframes placeholderAnimate {
    0% { background-position: 200% 0; }
    100% { background-position: -200% 0; }
}

/* Spinner */
@keyframes spinLeft {
    0% {
        transform: rotate(20deg);
    }
    50% {
        transform: rotate(160deg);
    }
    100% {
        transform: rotate(20deg);
    }
}
@keyframes spinRight {
    0% {
        transform: rotate(160deg);
    }
    50% {
        transform: rotate(20deg);
    }
    100% {
        transform: rotate(160deg);
    }
}
@keyframes spin {
    0% {
        transform: rotate(0deg);
    }
    100% {
        transform: rotate(2520deg);
    }
}

.spinner {
    position: relative;
    width: 1.5em;
    height: 1.5em;
    margin: auto;
    animation: spin 10s linear infinite;
}
.spinner::before {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    right: 0;
}
.right, .rightWrapper, .left, .leftWrapper {
    position: absolute;
    top: 0;
    overflow: hidden;
    width: .75em;
    height: 1.5em;
}
.left, .leftWrapper {
    left: 0;
}
.right {
    left: -12px;
}
.rightWrapper {
    right: 0;
}
.circle {
    border: .125em solid #A8B6B8;
    width: 1.25em; /* 1.5em - 2*0.125em border */
    height: 1.25em; /* 1.5em - 2*0.125em border */
    border-radius: 0.75em; /* 0.5*1.5em spinner size 8 */
}
.left {
    transform-origin: 100% 50%;
    animation: spinLeft 2.5s cubic-bezier(.2,0,.8,1) infinite;
}
.right {
    transform-origin: 100% 50%;
    animation: spinRight 2.5s cubic-bezier(.2,0,.8,1) infinite;
}
`,la="data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMjQiIGhlaWdodD0iMjQiIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0ibm9uZSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KPHBhdGggZmlsbC1ydWxlPSJldmVub2RkIiBjbGlwLXJ1bGU9ImV2ZW5vZGQiIGQ9Ik03IDIuNUgxN0MxNy44Mjg0IDIuNSAxOC41IDMuMTcxNTcgMTguNSA0VjIwQzE4LjUgMjAuODI4NCAxNy44Mjg0IDIxLjUgMTcgMjEuNUg3QzYuMTcxNTcgMjEuNSA1LjUgMjAuODI4NCA1LjUgMjBWNEM1LjUgMy4xNzE1NyA2LjE3MTU3IDIuNSA3IDIuNVpNMyA0QzMgMS43OTA4NiA0Ljc5MDg2IDAgNyAwSDE3QzE5LjIwOTEgMCAyMSAxLjc5MDg2IDIxIDRWMjBDMjEgMjIuMjA5MSAxOS4yMDkxIDI0IDE3IDI0SDdDNC43OTA4NiAyNCAzIDIyLjIwOTEgMyAyMFY0Wk0xMSA0LjYxNTM4QzEwLjQ0NzcgNC42MTUzOCAxMCA1LjA2MzEgMTAgNS42MTUzOFY2LjM4NDYyQzEwIDYuOTM2OSAxMC40NDc3IDcuMzg0NjIgMTEgNy4zODQ2MkgxM0MxMy41NTIzIDcuMzg0NjIgMTQgNi45MzY5IDE0IDYuMzg0NjJWNS42MTUzOEMxNCA1LjA2MzEgMTMuNTUyMyA0LjYxNTM4IDEzIDQuNjE1MzhIMTFaIiBmaWxsPSIjRENCOEZGIi8+Cjwvc3ZnPgo=",x_="Mobile Wallet Adapter",M_="Remote Mobile Wallet Adapter",da=64,_a=[$,k,ve,ye],U_=3e4;function D(t){return t instanceof Error?t.message:"Unknown error"}var P_=class{#e={};#n="1.0.0";#r=x_;#d="https://solanamobile.com/wallets";#_=la;#s;#t;#a;#o=!1;#u=0;#c=[];#N;#R;#S;get version(){return this.#n}get name(){return this.#r}get url(){return this.#d}get icon(){return this.#_}get chains(){return this.#c}get features(){return{[Ze]:{version:"1.0.0",connect:this.#m},[Qe]:{version:"1.0.0",disconnect:this.#I},[et]:{version:"1.0.0",on:this.#f},[ve]:{version:"1.0.0",signMessage:this.#b},[ye]:{version:"1.0.0",signIn:this.#y},...this.#R}}get accounts(){return this.#t?.accounts??[]}constructor(t){this.#a=t.authorizationCache,this.#s=t.appIdentity,this.#c=t.chains,this.#N=t.chainSelector,this.#S=t.onWalletNotFound,this.#R={[$]:{version:"1.0.0",supportedTransactionVersions:["legacy",0],signAndSendTransaction:this.#w},[k]:{version:"1.0.0",supportedTransactionVersions:["legacy",0],signTransaction:this.#L}}}get connected(){return!!this.#t}get isAuthorized(){return!!this.#t}get currentAuthorization(){return this.#t}get cachedAuthorizationResult(){return this.#a.get()}#f=(t,e)=>(this.#e[t]?.push(e)||(this.#e[t]=[e]),()=>this.#D(t,e));#i(t,...e){this.#e[t]?.forEach(n=>n.apply(null,e))}#D(t,e){this.#e[t]=this.#e[t]?.filter(n=>e!==n)}#m=async({silent:t}={})=>{if(this.#o||this.connected)return{accounts:this.accounts};this.#o=!0;try{if(t){let e=await this.#a.get();if(e)await this.#p(e.capabilities),await this.#A(e);else return{accounts:this.accounts}}else await this.#T()}catch(e){throw new Error(D(e),{cause:e})}finally{this.#o=!1}return{accounts:this.accounts}};#T=async t=>{try{let e=await this.#a.get();if(e)return this.#A(e),e;let n=await this.#N.select(this.#c);return await this.#l(async a=>{let[r,o]=await Promise.all([a.getCapabilities(),a.authorize({chain:n,identity:this.#s,sign_in_payload:t})]),i=this.#h(o.accounts),s={...o,accounts:i,chain:n,capabilities:r};return Promise.all([this.#p(r),this.#a.set(s),this.#A(s)]),s})}catch(e){throw new Error(D(e),{cause:e})}};#A=async t=>{let e=this.#t==null||this.#t?.accounts.length!==t.accounts.length||this.#t.accounts.some((n,a)=>n.address!==t.accounts[a].address);this.#t=t,e&&this.#i("change",{accounts:this.accounts})};#p=async t=>{let e=t.features.includes("solana:signTransactions"),n=t.supports_sign_and_send_transactions,a=$ in this.features!==n||k in this.features!==e;this.#R={...(n||!n&&!e)&&{[$]:{version:"1.0.0",supportedTransactionVersions:["legacy",0],signAndSendTransaction:this.#w}},...e&&{[k]:{version:"1.0.0",supportedTransactionVersions:["legacy",0],signTransaction:this.#L}}},a&&this.#i("change",{features:this.features})};#E=async(t,e,n)=>{try{let[a,r]=await Promise.all([this.#t?.capabilities??await t.getCapabilities(),t.authorize({auth_token:e,identity:this.#s,chain:n})]),o=this.#h(r.accounts),i={...r,accounts:o,chain:n,capabilities:a};Promise.all([this.#a.set(i),this.#A(i)])}catch(a){throw this.#I(),new Error(D(a),{cause:a})}};#I=async()=>{this.#a.clear(),this.#o=!1,this.#u++,this.#t=void 0,this.#i("change",{accounts:this.accounts})};#l=async t=>{let e=this.#t?.wallet_uri_base,n=e?{baseUri:e}:void 0,a=this.#u,r=new b_;try{let o=!0,i,s=await Promise.race([ca().then(async()=>{r.init();let{wallet:c,close:l}=await an(n);o=!1,r.addEventListener("close",N=>{N&&l()}),r.open();let d=await t(await c);return r.close(),l(),d}),new Promise((c,l)=>{i=setTimeout(()=>{o&&l(new O(S.ERROR_ASSOCIATION_CANCELLED,"Wallet connection timed out",{event:void 0}))},U_)})]);return clearTimeout(i),s}catch(o){throw r.close(),this.#u!==a&&await new Promise(()=>{}),o instanceof Error&&o.name==="SolanaMobileWalletAdapterError"&&o.code==="ERROR_WALLET_NOT_FOUND"&&await this.#S(this),o}};#O=()=>{if(!this.#t)throw new Error("Wallet not connected");return{authToken:this.#t.auth_token,chain:this.#t.chain}};#h=t=>t.map(e=>{let n=I(e.address);return{address:X(n),publicKey:n,label:e.label,icon:e.icon,chains:e.chains??this.#c,features:e.features??_a}});#g=async t=>{let{authToken:e,chain:n}=this.#O();try{let a=t.map(r=>P(r));return await this.#l(async r=>(await this.#E(r,e,n),(await r.signTransactions({payloads:a})).signed_payloads.map(I)))}catch(a){throw new Error(D(a),{cause:a})}};#C=async(t,e)=>{let{authToken:n,chain:a}=this.#O();try{return await this.#l(async r=>{let[o]=await Promise.all([r.getCapabilities(),this.#E(r,n,a)]);if(o.supports_sign_and_send_transactions){let i=P(t);return(await r.signAndSendTransactions({...e,payloads:[i]})).signatures.map(I)[0]}else throw new Error("connected wallet does not support signAndSendTransaction")})}catch(r){throw new Error(D(r),{cause:r})}};#w=async(...t)=>{let e=[];for(let n of t){let a=await this.#C(n.transaction,n.options);e.push({signature:a})}return e};#L=async(...t)=>(await this.#g(t.map(({transaction:e})=>e))).map(e=>({signedTransaction:e}));#b=async(...t)=>{let{authToken:e,chain:n}=this.#O(),a=t.map(({account:o})=>P(new Uint8Array(o.publicKey))),r=t.map(({message:o})=>P(o));try{return await this.#l(async o=>(await this.#E(o,e,n),(await o.signMessages({addresses:a,payloads:r})).signed_payloads.map(I).map(i=>({signedMessage:i,signature:i.slice(-da)}))))}catch(o){throw new Error(D(o),{cause:o})}};#y=async(...t)=>{let e=[];if(t.length>1)for(let n of t)e.push(await this.#v(n));else return[await this.#v(t[0])];return e};#v=async t=>{this.#o=!0;try{let e=await this.#T({...t,domain:t?.domain??window.location.host});if(!e.sign_in_result)throw new Error("Sign in failed, no sign in result returned by wallet");let n=e.sign_in_result.address,a=e.accounts.find(r=>r.address==n);return{account:{...a??{address:X(I(n))},publicKey:I(n),chains:a?.chains??this.#c,features:a?.features??e.capabilities.features},signedMessage:I(e.sign_in_result.signed_message),signature:I(e.sign_in_result.signature)}}catch(e){throw new Error(D(e),{cause:e})}finally{this.#o=!1}}},B_=class{#e={};#n="1.0.0";#r=M_;#d="https://solanamobile.com/wallets";#_=la;#s;#t;#a;#o=!1;#u=0;#c=[];#N;#R;#S;#f;#i;get version(){return this.#n}get name(){return this.#r}get url(){return this.#d}get icon(){return this.#_}get chains(){return this.#c}get features(){return{[Ze]:{version:"1.0.0",connect:this.#A},[Qe]:{version:"1.0.0",disconnect:this.#O},[et]:{version:"1.0.0",on:this.#D},[ve]:{version:"1.0.0",signMessage:this.#v},[ye]:{version:"1.0.0",signIn:this.#M},...this.#R}}get accounts(){return this.#t?.accounts??[]}constructor(t){this.#a=t.authorizationCache,this.#s=t.appIdentity,this.#c=t.chains,this.#N=t.chainSelector,this.#f=t.remoteHostAuthority,this.#S=t.onWalletNotFound,this.#R={[$]:{version:"1.0.0",supportedTransactionVersions:["legacy",0],signAndSendTransaction:this.#b},[k]:{version:"1.0.0",supportedTransactionVersions:["legacy",0],signTransaction:this.#y}}}get connected(){return!!this.#i&&!!this.#t}get isAuthorized(){return!!this.#t}get currentAuthorization(){return this.#t}get cachedAuthorizationResult(){return this.#a.get()}#D=(t,e)=>(this.#e[t]?.push(e)||(this.#e[t]=[e]),()=>this.#T(t,e));#m(t,...e){this.#e[t]?.forEach(n=>n.apply(null,e))}#T(t,e){this.#e[t]=this.#e[t]?.filter(n=>e!==n)}#A=async(t={})=>{if(this.#o||this.connected)return{accounts:this.accounts};this.#o=!0;try{await this.#p()}catch(e){throw new Error(D(e),{cause:e})}finally{this.#o=!1}return{accounts:this.accounts}};#p=async t=>{try{let e=await this.#a.get();if(e)return this.#E(e),e;this.#i&&(this.#i=void 0);let n=await this.#N.select(this.#c);return await this.#h(async a=>{let[r,o]=await Promise.all([a.getCapabilities(),a.authorize({chain:n,identity:this.#s,sign_in_payload:t})]),i=this.#C(o.accounts),s={...o,accounts:i,chain:n,capabilities:r};return Promise.all([this.#I(r),this.#a.set(s),this.#E(s)]),s})}catch(e){throw new Error(D(e),{cause:e})}};#E=async t=>{let e=this.#t==null||this.#t?.accounts.length!==t.accounts.length||this.#t.accounts.some((n,a)=>n.address!==t.accounts[a].address);this.#t=t,e&&this.#m("change",{accounts:this.accounts})};#I=async t=>{let e=t.features.includes("solana:signTransactions"),n=t.supports_sign_and_send_transactions||t.features.includes("solana:signAndSendTransaction"),a=$ in this.features!==n||k in this.features!==e;this.#R={...n&&{[$]:{version:"1.0.0",supportedTransactionVersions:t.supported_transaction_versions,signAndSendTransaction:this.#b}},...e&&{[k]:{version:"1.0.0",supportedTransactionVersions:t.supported_transaction_versions,signTransaction:this.#y}}},a&&this.#m("change",{features:this.features})};#l=async(t,e,n)=>{try{let[a,r]=await Promise.all([this.#t?.capabilities??await t.getCapabilities(),t.authorize({auth_token:e,identity:this.#s,chain:n})]),o=this.#C(r.accounts),i={...r,accounts:o,chain:n,capabilities:a};Promise.all([this.#a.set(i),this.#E(i)])}catch(a){throw this.#O(),new Error(D(a),{cause:a})}};#O=async()=>{this.#i?.close(),this.#a.clear(),this.#o=!1,this.#u++,this.#t=void 0,this.#i=void 0,this.#m("change",{accounts:this.accounts})};#h=async t=>{let e=this.#t?.wallet_uri_base,n={...e?{baseUri:e}:void 0,remoteHostAuthority:this.#f},a=this.#u,r=new y_;if(this.#i)return t(this.#i.wallet);try{r.init(),r.open();let{associationUrl:o,close:i,wallet:s}=await rn(n),c=r.addEventListener("close",l=>{l&&i()});return r.populateQRCode(o.toString()),this.#i={close:i,wallet:await s},c(),r.close(),await t(this.#i.wallet)}catch(o){throw r.close(),this.#u!==a&&await new Promise(()=>{}),o instanceof Error&&o.name==="SolanaMobileWalletAdapterError"&&o.code==="ERROR_WALLET_NOT_FOUND"&&await this.#S(this),o}};#g=()=>{if(!this.#t)throw new Error("Wallet not connected");return{authToken:this.#t.auth_token,chain:this.#t.chain}};#C=t=>t.map(e=>{let n=I(e.address);return{address:X(n),publicKey:n,label:e.label,icon:e.icon,chains:e.chains??this.#c,features:e.features??_a}});#w=async t=>{let{authToken:e,chain:n}=this.#g();try{return await this.#h(async a=>(await this.#l(a,e,n),(await a.signTransactions({payloads:t.map(P)})).signed_payloads.map(I)))}catch(a){throw new Error(D(a),{cause:a})}};#L=async(t,e)=>{let{authToken:n,chain:a}=this.#g();try{return await this.#h(async r=>{let[o]=await Promise.all([r.getCapabilities(),this.#l(r,n,a)]);if(o.supports_sign_and_send_transactions)return(await r.signAndSendTransactions({...e,payloads:[P(t)]})).signatures.map(I)[0];throw new Error("connected wallet does not support signAndSendTransaction")})}catch(r){throw new Error(D(r),{cause:r})}};#b=async(...t)=>{let e=[];for(let n of t){let a=await this.#L(n.transaction,n.options);e.push({signature:a})}return e};#y=async(...t)=>(await this.#w(t.map(({transaction:e})=>e))).map(e=>({signedTransaction:e}));#v=async(...t)=>{let{authToken:e,chain:n}=this.#g(),a=t.map(({account:o})=>P(new Uint8Array(o.publicKey))),r=t.map(({message:o})=>P(o));try{return await this.#h(async o=>(await this.#l(o,e,n),(await o.signMessages({addresses:a,payloads:r})).signed_payloads.map(I).map(i=>({signedMessage:i,signature:i.slice(-da)}))))}catch(o){throw new Error(D(o),{cause:o})}};#M=async(...t)=>{let e=[];if(t.length>1)for(let n of t)e.push(await this.#x(n));else return[await this.#x(t[0])];return e};#x=async t=>{this.#o=!0;try{let e=await this.#p({...t,domain:t?.domain??window.location.host});if(!e.sign_in_result)throw new Error("Sign in failed, no sign in result returned by wallet");let n=e.sign_in_result.address,a=e.accounts.find(r=>r.address==n);return{account:{...a??{address:X(I(n))},publicKey:I(n),chains:a?.chains??this.#c,features:a?.features??e.capabilities.features},signedMessage:I(e.sign_in_result.signed_message),signature:I(e.sign_in_result.signature)}}catch(e){throw new Error(D(e),{cause:e})}finally{this.#o=!1}}};function Qu(t){if(typeof window>"u"){console.warn("MWA not registered: no window object");return}if(!window.isSecureContext){console.warn("MWA not registered: secure context required (https)");return}let e=navigator.userAgent;f_()&&(!I_(e)||sa(e))?je(new P_(t)):T_()&&t.remoteHostAuthority!==void 0&&je(new B_({...t,remoteHostAuthority:t.remoteHostAuthority}))}export{P_ as LocalSolanaMobileWalletAdapterWallet,B_ as RemoteSolanaMobileWalletAdapterWallet,M_ as SolanaMobileWalletAdapterRemoteWalletName,x_ as SolanaMobileWalletAdapterWalletName,ju as createDefaultAuthorizationCache,Ju as createDefaultChainSelector,Zu as createDefaultWalletNotFoundHandler,R_ as defaultErrorModalWalletNotFoundHandler,Qu as registerMwa};
