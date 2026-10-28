if (typeof(L) != "undefined") {

L.BingLayer = L.TileLayer.extend({
	options: {
		subdomains: [0, 1, 2, 3],
		type: 'Aerial',
		attribution: 'Bing',
		culture: ''
	},

	initialize: function(key, options) {
		L.Util.setOptions(this, options);

		this._key = key;
		this._url = null;
		this.meta = {};
		this.loadMetadata();
	},

	tile2quad: function(x, y, z) {
		var quad = '';
		for (var i = z; i > 0; i--) {
			var digit = 0;
			var mask = 1 << (i - 1);
			if ((x & mask) != 0) digit += 1;
			if ((y & mask) != 0) digit += 2;
			quad = quad + digit;
		}
		return quad;
	},

	getTileUrl: function(p) {
		var z = this._getZoomForUrl();
		var subdomains = this.options.subdomains,
			s = this.options.subdomains[Math.abs((p.x + p.y) % subdomains.length)];
		return this._url.replace('{subdomain}', s)
				.replace('{quadkey}', this.tile2quad(p.x, p.y, z))
				.replace('{culture}', this.options.culture);
	},

	loadMetadata: function() {
		var _this = this;
		var cbid = '_bing_metadata_' + L.Util.stamp(this);
		window[cbid] = function (meta) {
			_this.meta = meta;
			window[cbid] = undefined;
			var e = document.getElementById(cbid);
			e.parentNode.removeChild(e);
			if (meta.errorDetails) {
				alert("Got metadata" + meta.errorDetails);
				return;
			}
			_this.initMetadata();
		};
		var url = "http://dev.virtualearth.net/REST/v1/Imagery/Metadata/" + this.options.type + "?include=ImageryProviders&jsonp=" + cbid + "&key=" + this._key;
		var script = document.createElement("script");
		script.type = "text/javascript";
		script.src = url;
		script.id = cbid;
		document.getElementsByTagName("head")[0].appendChild(script);
	},

	initMetadata: function() {
		var r = this.meta.resourceSets[0].resources[0];
		this.options.subdomains = r.imageUrlSubdomains;
		this._url = r.imageUrl;
		this._providers = [];
		for (var i = 0; i < r.imageryProviders.length; i++) {
			var p = r.imageryProviders[i];
			for (var j = 0; j < p.coverageAreas.length; j++) {
				var c = p.coverageAreas[j];
				var coverage = {zoomMin: c.zoomMin, zoomMax: c.zoomMax, active: false};
				var bounds = new L.LatLngBounds(
						new L.LatLng(c.bbox[0]+0.01, c.bbox[1]+0.01),
						new L.LatLng(c.bbox[2]-0.01, c.bbox[3]-0.01)
				);
				coverage.bounds = bounds;
				coverage.attrib = p.attribution;
				this._providers.push(coverage);
			}
		}
		this._update();
	},

	_update: function() {
		if (this._url == null || !this._map) return;
		L.TileLayer.prototype._update.apply(this, []);
	},

	onRemove: function(map) {
		for (var i = 0; i < this._providers.length; i++) {
			var p = this._providers[i];
			if (p.active) {
				this._map.attributionControl.removeAttribution(p.attrib);
				p.active = false;
			}
		}
		L.TileLayer.prototype.onRemove.apply(this, [map]);
	}
});

L.NavitelLayer = L.TileLayer.extend({
	options: {
		attribution: 'Navitel',
		minZoom: 3,
		maxZoom: 17
	},

	getTileUrl: function(p) {
		var z = this._getZoomForUrl();
		var ss = '' + (1 + Math.abs((p.x + p.y) % 4));
		var sx = '' + p.x;
		while (sx.length < 8)
			sx = '0' + sx;
		var sy = '' + (Math.pow(2, z) - 1 - p.y);
		while (sy.length < 8)
			sy = '0' + sy;
		var sz = '' + z;
		while (sz.length < 2)
			sz = '0' + sz;
		return "http://m0" + ss + ".navitel.su/navitms.fcgi?t=" + sx + "," + sy + "," + sz + "&hl=ru";
	}
});

L.YahooLayer = L.TileLayer.extend({
	options: {
		attribution: 'Yahoo',
		type: "Road",
		maxZoom: 17
	},

	initialize: function(options) {
		L.Util.setOptions(this, options);
	},

	getTileUrl: function(p) {
		var z = this._getZoomForUrl();
		var url = "http://maps"+((p.x+p.y)%3+1)+".yimg.com";
		if (this.options.type == "Sat")
			url = url + "/ae/ximg";
		else
			url = url + "/hx/tl";
		url = url + "?s=256&t=a&r=1&x="+p.x+"&y="+(((Math.floor(Math.pow(2,z))/ 2)-1)-p.y)+'&z='+(z+1);
		return url;
	}
});

L.gmtLayer = L.TileLayer.extend({
	options: {
		host: '',
		usermap: ''
	},

	getTileUrl: function(p, z) {
		var z = this._getZoomForUrl();
		var dx = Math.floor(p.x / 1024);
		var dy = Math.floor(p.y / 1024);
		var dz = z;
		return "http://"+this.options.host+"/" + this.options.usermap + "/"+dz+"/"+p.x+"/"+p.y+".jpg";
	},

	_update: function() {
		if (this._url == null || !this._map) return;
		L.TileLayer.prototype._update.apply(this, []);
	},

	onRemove: function(map) {
		L.TileLayer.prototype.onRemove.apply(this, [map]);
	}

});

}
