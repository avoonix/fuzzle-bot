<script setup lang="ts">
import { useFetch } from '@vueuse/core';
import { computed, ref, watch } from 'vue';
import { useRoute } from 'vue-router';

const route = useRoute();

interface StickerPub {
  id: string,
  set_id: string,
  similarity: number
}

const url = computed(() => `/api/stickers/${route.params.stickerId}/similar`)

const { data, error, execute: refetch } = useFetch(url, { refetch: true, updateDataOnError: true }).json<StickerPub[]>()

watch(error, () => console.log(error))

const maxSimilarity = ref(0.9)

const filteredSticker = computed(() => (data.value || []).filter(s => s.similarity >= maxSimilarity.value))

watch(url, () => isBanned.value = false);

const isBanned = ref(false)

const toggleBan = async () => {
  if (isBanned.value) {
    const { data, error } = await useFetch(`/api/stickers/${route.params.stickerId}/unban`).post()
    console.log(data, error)
    if (!error.value) {
      isBanned.value = false;
    }
  } else {
    const { data, error } = await useFetch(`/api/stickers/${route.params.stickerId}/ban`).post({
      clip_max_match_distance: maxSimilarity.value
    })
    console.log(data, error)
    if (!error.value) {
      isBanned.value = true;
    }
  }
}

</script>

<template>
  <div class="asdf">
    <h1>This is an ban similar view</h1>
     <v-slider
        v-model="maxSimilarity"
        thumb-label
        min="0.7"
        max="1"
        step="0.005"
      ></v-slider>
    {{ route.params.setId }}
    asdf
    {{ error }}

    <v-btn @click="toggleBan" v-if="isBanned" color="error">
      Unban
    </v-btn>
    <v-btn @click="toggleBan" v-else color="success">
      Ban
    </v-btn>


      <div v-if="data">
        <div v-for="sticker of filteredSticker">
          <img :src="`/files/stickers/${sticker.id}/thumbnail.png`" loading="lazy" width="128" height="128" />
          {{ sticker }}
          <v-btn :to="{name: 'banSimilarView', params: {setId: sticker.set_id, stickerId: sticker.id }}">
            ban similar view
          </v-btn>
        </div>
      </div>
  </div>
</template>

<style>

</style>
